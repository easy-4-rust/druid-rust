use super::contract_lease::ContractLease;
use super::stateful_connection::StatefulConnection;
use super::ConnectionDefaults;
use crate::core::ExternalConnectionLease;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::statement::ResultSetHoldability;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 捕获、比较、复位全部原状态，并保留显式隔离级别不复位选项。
/// 对应 Java: DruidConnectionHolder#reset。
#[tokio::test]
async fn task6_connection_defaults_capture_compare_and_reset_complete_state() {
    let drop_count = Arc::new(AtomicUsize::new(0));
    let mut connection = ExternalConnectionLease::new(ContractLease {
        connection: Box::new(StatefulConnection {
            auto_commit: true,
            read_only: false,
            isolation: TransactionIsolation::from_jdbc(2),
            holdability: ResultSetHoldability::DriverValue(0),
            catalog: "catalog_a".to_owned(),
            schema: "schema_a".to_owned(),
        }),
        drop_count: Arc::clone(&drop_count),
    });
    let defaults = ConnectionDefaults::capture_standard(&mut connection)
        .await
        .unwrap();

    assert!(defaults.auto_commit());
    assert!(!defaults.read_only());
    assert_eq!(defaults.transaction_isolation(), 2);
    assert_eq!(defaults.holdability(), 0);
    assert!(!defaults.standard_needs_reset(&connection, false).unwrap());

    connection
        .set_read_only(true)
        .await
        .expect("read only must change");
    connection
        .set_holdability(ResultSetHoldability::HoldOverCommit)
        .await
        .expect("holdability must change");
    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .expect("isolation must change");
    connection
        .set_auto_commit(false)
        .await
        .expect("auto commit must change");

    assert!(defaults.standard_needs_reset(&connection, false).unwrap());
    defaults
        .reset_standard_state(&mut connection, true, false)
        .await
        .expect("all supported connection state must reset");
    assert!(!defaults.standard_needs_reset(&connection, false).unwrap());
    assert!(connection.auto_commit().unwrap());
    assert!(!connection.read_only().unwrap());
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 2);
    assert_eq!(connection.holdability().unwrap().rdbc_code(), 0);

    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .expect("isolation must change again");
    assert!(!defaults.standard_needs_reset(&connection, true).unwrap());
    defaults
        .reset_standard_state(&mut connection, true, true)
        .await
        .expect("isolation preservation must skip reset");
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);

    drop(connection);
    assert_eq!(drop_count.load(Ordering::Relaxed), 1);
}
