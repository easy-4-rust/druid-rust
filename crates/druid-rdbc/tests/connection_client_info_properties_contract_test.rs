//! JDBC 客户端信息的整组替换与无损失败合同。
#[path = "client_info_support/connection.rs"]
mod connection;
use connection::ClientInfoConnection;
use druid_rdbc::connection::Connection;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
fn properties(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}
#[tokio::test]
async fn bulk_properties_replace_clear_and_return_independent_snapshots() {
    let mut connection =
        ClientInfoConnection::new(properties(&[("old", "remove"), ("keep", "before")]));
    let requested = properties(&[("keep", "after"), ("new", "")]);
    connection
        .set_client_info_properties(Some(&requested))
        .await
        .unwrap();
    let mut snapshot = connection.client_info_properties().await.unwrap();
    assert_eq!(snapshot, requested);
    snapshot.insert("caller-only".to_owned(), "changed".to_owned());
    assert_eq!(connection.client_info("caller-only").unwrap(), None);
    assert_eq!(connection.client_info("new").unwrap(), Some(String::new()));
    connection.set_client_info("new", None).await.unwrap();
    assert_eq!(connection.client_info("new").unwrap(), None);
    connection
        .set_client_info_properties(Some(&HashMap::new()))
        .await
        .unwrap();
    assert!(connection
        .client_info_properties()
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn partial_bulk_failure_retains_driver_status_unattempted_names_and_cause() {
    let mut connection = ClientInfoConnection::new(properties(&[("old", "before")]));
    *connection.fail_name.lock().unwrap() = Some("b".to_owned());
    let error = connection
        .set_client_info_properties(Some(&properties(&[
            ("a", "private-value"),
            ("b", "bad"),
            ("c", "later"),
        ])))
        .await
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
    let failed = error.failed_client_info_properties().unwrap();
    assert_eq!(
        failed,
        &properties(&[
            ("b", "REASON_VALUE_INVALID"),
            ("c", "REASON_UNKNOWN"),
            ("old", "REASON_UNKNOWN")
        ])
    );
    assert_eq!(error.sql_exception().unwrap().error_code(), 77);
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22023"));
    assert!(std::error::Error::source(&error).is_some());
    assert!(!error.to_string().contains("private-value"));
    assert_eq!(
        connection.client_info("a").unwrap().as_deref(),
        Some("private-value")
    );
    assert_eq!(
        connection.client_info("old").unwrap().as_deref(),
        Some("before")
    );
    assert_eq!(connection.writes.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn null_or_unavailable_snapshot_never_fabricates_bulk_success() {
    let mut connection = ClientInfoConnection::new(HashMap::new());
    let null = connection
        .set_client_info_properties(None)
        .await
        .unwrap_err();
    assert_eq!(null.class_name(), "java.sql.SQLClientInfoException");
    assert!(null.failed_client_info_properties().unwrap().is_empty());
    assert_eq!(connection.snapshot_calls.load(Ordering::SeqCst), 0);
    assert_eq!(connection.writes.load(Ordering::SeqCst), 0);
    connection.snapshot_supported.store(false, Ordering::SeqCst);
    let error = connection
        .set_client_info_properties(Some(&properties(&[("name", "value")])))
        .await
        .unwrap_err();
    assert_eq!(
        error.failed_client_info_properties().unwrap()["name"],
        "REASON_UNKNOWN"
    );
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("0A000"));
    assert_eq!(connection.writes.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn closed_setter_retains_client_info_type_and_requested_property_identity() {
    let mut connection = ClientInfoConnection::new(HashMap::new());
    connection.close().await.unwrap();
    let error = connection
        .set_client_info_properties(Some(&properties(&[("ApplicationName", "private-value")])))
        .await
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("08003"));
    assert_eq!(
        error.failed_client_info_properties().unwrap()["ApplicationName"],
        "REASON_UNKNOWN"
    );
}
