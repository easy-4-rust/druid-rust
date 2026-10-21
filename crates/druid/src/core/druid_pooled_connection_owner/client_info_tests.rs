//! 标准 owner 的客户端属性委托和逻辑租约隔离。
#[path = "../../../../druid-rdbc/tests/client_info_support/connection.rs"]
mod connection;
use super::DruidPooledConnectionOwner;
use crate::core::{DruidConnectionHolder, DruidPooledConnection};
use connection::ClientInfoConnection;
use druid_rdbc::data_source::PooledConnection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[tokio::test]
async fn owner_reissues_clean_properties_and_rejects_closed_logical_setters() {
    let baseline = HashMap::from([("ApplicationName".to_owned(), "baseline".to_owned())]);
    let raw = ClientInfoConnection::new(baseline.clone());
    let values = Arc::clone(&raw.values);
    let returned = Arc::new(Mutex::new(Vec::new()));
    let callback = Arc::clone(&returned);
    let holder =
        DruidConnectionHolder::with_standard_connection(Box::new(raw), 73, Duration::ZERO, 0)
            .await
            .unwrap();
    let pooled = DruidPooledConnection::with_holder(
        holder,
        "owner-client-info".to_owned(),
        None,
        false,
        Box::new(move |_, disposition| {
            callback.lock().unwrap().push(disposition);
            false
        }),
    );
    let mut owner = DruidPooledConnectionOwner::new(pooled);
    let mut first = owner.connection().await.unwrap();
    first
        .set_client_info_properties(Some(&HashMap::from([(
            "ClientUser".to_owned(),
            "private-user".to_owned(),
        )])))
        .await
        .unwrap();
    first.close().await.unwrap();
    assert_eq!(*values.lock().unwrap(), baseline);
    let closed = first.set_client_info("ClientUser", None).await.unwrap_err();
    assert_eq!(closed.class_name(), "java.sql.SQLClientInfoException");
    let mut second = owner.connection().await.unwrap();
    assert_eq!(second.client_info_properties().await.unwrap(), baseline);
    second.close().await.unwrap();
    owner.close().await.unwrap();
    assert_eq!(returned.lock().unwrap().len(), 1);
    assert!(returned.lock().unwrap()[0].is_reusable());
}
