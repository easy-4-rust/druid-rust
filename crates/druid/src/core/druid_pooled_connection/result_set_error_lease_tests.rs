//! 完整保留原结果集错误、创建语句异常计数和底层连接复用后的旧租约拒绝。

use super::DruidPooledConnection;
use crate::core::{DruidConnectionHolder, DruidPooledStatement};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::types::RdbcTargetType;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 验证完整原游标错误优先级、实际语句三次异常及同一底层连接的旧租约隔离。
/// 对应 Java: DruidPooledResultSet#checkException 与 DruidPooledConnection#close。
#[tokio::test]
async fn task6_result_set_errors_typed_getters_and_old_lease_are_observable() {
    let returned: Arc<Mutex<Option<Box<dyn Connection>>>> = Arc::new(Mutex::new(None));
    let returned_from_callback = Arc::clone(&returned);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let physical = factory.create().await.unwrap();
    let physical_identity =
        std::ptr::from_ref::<dyn Connection>(physical.as_ref()).cast::<()>() as usize;
    let holder = DruidConnectionHolder::with_standard_connection(physical, 19, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "result-set-error-lease".to_owned(),
        None,
        false,
        false,
        Box::new(move |mut holder, _| {
            *returned_from_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                holder.take_standard_connection();
            true
        }),
    );

    // 保留具体创建语句用于原异常计数观察；使用真实连接的策略、租约和资源登记。
    let resources = connection
        .holder
        .as_ref()
        .unwrap()
        .standard_resource_registry();
    let policy = Arc::clone(connection.standard_policy.as_ref().unwrap());
    let admission = resources
        .begin_admission(policy.resource_generation())
        .unwrap();
    let raw_statement = connection
        .standard_mut()
        .unwrap()
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        19,
        connection.next_statement_id(),
        Arc::clone(&connection.result_set_id_seed),
        Arc::clone(&connection.metadata_id_seed),
        Arc::clone(&connection.lease_active),
        None,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut result_set = statement.execute_query("SELECT 1").await.unwrap();
    let column = ColumnIndex::new(1).unwrap();

    let first_read = result_set.object(column);
    assert!(
        matches!(
            &first_read,
            Err(RdbcError::Other(message))
                if message == "result set cursor is not positioned on a row"
        ),
        "实际未定位读取结果：{first_read:?}"
    );
    assert!(matches!(
        result_set.object_as(column, &RdbcTargetType::Custom("X".to_string())),
        Err(RdbcError::Other(message))
            if message == "result set cursor is not positioned on a row"
    ));
    assert!(result_set.next().await.unwrap());
    assert!(matches!(
        result_set.object_as(column, &RdbcTargetType::Custom("X".to_string())),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(statement.exception_count(), 3);

    connection.close().await.unwrap();
    let physical = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("归还回调必须收到同一底层连接");
    assert_eq!(
        std::ptr::from_ref::<dyn Connection>(physical.as_ref()).cast::<()>() as usize,
        physical_identity
    );
    let holder = DruidConnectionHolder::with_standard_connection(physical, 19, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut next_lease = DruidPooledConnection::with_standard_holder(
        holder,
        "result-set-error-lease".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| false),
    );
    assert!(matches!(
        result_set.next().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    // 标准游标不再接受显式 connection 参数；以真实新租约可执行来证明隔离。
    assert_eq!(next_lease.id(), 19);
    assert_eq!(next_lease.fetch("SELECT 1", vec![]).await.unwrap().len(), 1);
    next_lease.close().await.unwrap();
}
