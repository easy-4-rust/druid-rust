//! 客户端属性在真实池化 holder 生命周期中的复位和污染隔离。
#[path = "../../druid-rdbc/tests/client_info_support/connection.rs"]
mod connection;
use connection::ClientInfoConnection;
use druid::core::{ConnectionRecycleDisposition, DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::connection::Connection;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
fn original() -> HashMap<String, String> {
    HashMap::from([("ApplicationName".to_owned(), "baseline".to_owned())])
}
async fn pooled(
    raw: ClientInfoConnection,
    returned: Arc<Mutex<Option<DruidConnectionHolder>>>,
    disposition: Arc<Mutex<Vec<ConnectionRecycleDisposition>>>,
) -> DruidPooledConnection {
    let holder =
        DruidConnectionHolder::with_standard_connection(Box::new(raw), 42, Duration::ZERO, 0)
            .await
            .unwrap();
    DruidPooledConnection::with_holder(
        holder,
        "client-info".to_owned(),
        None,
        false,
        Box::new(move |holder, result| {
            *returned.lock().unwrap() = Some(holder);
            disposition.lock().unwrap().push(result);
            false
        }),
    )
}
#[tokio::test]
async fn explicit_close_restores_client_properties_before_reusing_the_same_holder() {
    let raw = ClientInfoConnection::new(original());
    let values = Arc::clone(&raw.values);
    let returned = Arc::new(Mutex::new(None));
    let dispositions = Arc::new(Mutex::new(Vec::new()));
    let mut connection = pooled(raw, Arc::clone(&returned), Arc::clone(&dispositions)).await;
    connection
        .set_client_info_properties(Some(&HashMap::from([(
            "ClientUser".to_owned(),
            "private-user".to_owned(),
        )])))
        .await
        .unwrap();
    assert_eq!(connection.client_info_properties().await.unwrap().len(), 1);
    assert_eq!(connection.client_info("ApplicationName").unwrap(), None);
    connection.close().await.unwrap();
    assert_eq!(*values.lock().unwrap(), original());
    assert!(dispositions.lock().unwrap()[0].is_reusable());
    let holder = returned.lock().unwrap().take().unwrap();
    let mut second = DruidPooledConnection::with_holder(
        holder,
        "second".to_owned(),
        None,
        false,
        Box::new(|_, _| false),
    );
    assert_eq!(second.client_info_properties().await.unwrap(), original());
    second.close().await.unwrap();
}
#[tokio::test]
async fn dropping_a_dirty_lease_never_returns_private_properties_as_reusable() {
    let raw = ClientInfoConnection::new(original());
    let returned = Arc::new(Mutex::new(None));
    let dispositions = Arc::new(Mutex::new(Vec::new()));
    let mut connection = pooled(raw, returned, Arc::clone(&dispositions)).await;
    connection
        .set_client_info("ClientUser", Some("private-user"))
        .await
        .unwrap();
    drop(connection);
    assert_eq!(dispositions.lock().unwrap().len(), 1);
    assert!(!dispositions.lock().unwrap()[0].is_reusable());
}
#[tokio::test]
async fn cancelled_client_update_keeps_cleanup_obligation_until_close_restores_it() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let raw = ClientInfoConnection::new(original());
    raw.pause_next_write.store(true, Ordering::SeqCst);
    let values = Arc::clone(&raw.values);
    let dispositions = Arc::new(Mutex::new(Vec::new()));
    let mut connection = pooled(raw, Arc::new(Mutex::new(None)), Arc::clone(&dispositions)).await;
    let mut setting = Box::pin(connection.set_client_info("ClientUser", Some("private-user")));
    assert!(matches!(
        setting
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(setting);
    assert_eq!(
        values.lock().unwrap().get("ClientUser").map(String::as_str),
        Some("private-user")
    );
    connection.close().await.unwrap();
    assert_eq!(*values.lock().unwrap(), original());
    assert!(dispositions.lock().unwrap()[0].is_reusable());
}
#[tokio::test]
async fn missing_baseline_or_restore_failure_discards_instead_of_cross_lease_contamination() {
    for snapshot_supported in [false, true] {
        let raw = ClientInfoConnection::new(original());
        raw.snapshot_supported
            .store(snapshot_supported, Ordering::SeqCst);
        let rejected = Arc::clone(&raw.fail_name);
        let dispositions = Arc::new(Mutex::new(Vec::new()));
        let mut connection =
            pooled(raw, Arc::new(Mutex::new(None)), Arc::clone(&dispositions)).await;
        connection
            .set_client_info("ClientUser", Some("private-user"))
            .await
            .unwrap();
        if snapshot_supported {
            *rejected.lock().unwrap() = Some("ApplicationName".to_owned());
        }
        connection.close().await.unwrap();
        assert_eq!(dispositions.lock().unwrap().len(), 1);
        assert!(!dispositions.lock().unwrap()[0].is_reusable());
        let error = connection
            .set_client_info("ClientUser", None)
            .await
            .unwrap_err();
        assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
    }
}

#[tokio::test]
async fn external_pool_lease_delegates_property_snapshot_replace_and_closed_errors() {
    let raw: Box<dyn Connection> = Box::new(ClientInfoConnection::new(original()));
    let mut lease = druid::core::ExternalConnectionLease::new(Box::new(raw));
    let desired = HashMap::from([("ClientUser".to_owned(), "external".to_owned())]);
    lease
        .set_client_info_properties(Some(&desired))
        .await
        .unwrap();
    assert_eq!(lease.client_info_properties().await.unwrap(), desired);
    lease.close().await.unwrap();
    let error = lease
        .set_client_info_properties(Some(&HashMap::new()))
        .await
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("08003"));
}

#[tokio::test]
async fn pooled_proxy_preserves_partial_failure_without_relabelling_successful_properties() {
    let raw = ClientInfoConnection::new(HashMap::new());
    *raw.fail_name.lock().unwrap() = Some("b".to_owned());
    let rejected = Arc::clone(&raw.fail_name);
    let mut connection = pooled(
        raw,
        Arc::new(Mutex::new(None)),
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let error = connection
        .set_client_info_properties(Some(&HashMap::from([
            ("a".to_owned(), "success".to_owned()),
            ("b".to_owned(), "rejected".to_owned()),
            ("c".to_owned(), "pending".to_owned()),
        ])))
        .await
        .unwrap_err();
    assert_eq!(
        error.failed_client_info_properties().unwrap(),
        &HashMap::from([
            ("b".to_owned(), "REASON_VALUE_INVALID".to_owned()),
            ("c".to_owned(), "REASON_UNKNOWN".to_owned())
        ])
    );
    assert_eq!(
        connection.client_info("a").unwrap().as_deref(),
        Some("success")
    );
    *rejected.lock().unwrap() = None;
    connection.close().await.unwrap();
}
