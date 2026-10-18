//! 对照 Java Druid 1.2.28 `DruidPooledCallableStatementTest#test_wasNull_*`。

use druid::core::{ConnectionFactory, SqlException};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableOutParameter, CallableParameter, CallableStatement, ParameterIndex, PreparedStatement,
    Statement, StatementOptions,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 注入 `was_null` 普通 SQL 异常的底层 Callable。
struct ProbeCallable {
    fail_was_null: Arc<AtomicBool>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(ProbeCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for ProbeCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for ProbeCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("CALL parity")
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
    }
}

#[async_trait::async_trait]
impl CallableStatement for ProbeCallable {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        if self.fail_was_null.load(Ordering::Acquire) {
            Err(RdbcError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            Ok(false)
        }
    }
}

/// 为池提供同一个可控 Callable 错误状态。
struct ProbeConnection {
    fail_was_null: Arc<AtomicBool>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(ProbeConnection => Connection);

#[async_trait::async_trait]
impl Connection for ProbeConnection {
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(ProbeCallable {
            fail_was_null: Arc::clone(&self.fail_was_null),
            closed: false,
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Ok(!self.closed)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("callable-parity".to_owned())
    }
}

/// 创建受控底层连接。
struct ProbeFactory(Arc<AtomicBool>);

#[async_trait::async_trait]
impl ConnectionFactory for ProbeFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(ProbeConnection {
            fail_was_null: Arc::clone(&self.0),
            closed: false,
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[tokio::test]
async fn java_callable_was_null_sql_exception_counts_once_without_new_execution() {
    let fail_was_null = Arc::new(AtomicBool::new(false));
    let pool = DruidPool::builder()
        .name("java-callable-was-null-parity")
        .driver_name("callable-parity")
        .factory(Arc::new(ProbeFactory(Arc::clone(&fail_was_null))) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("pool");
    let mut connection = pool.get().await.expect("borrow");
    let mut callable = connection
        .prepare_call("CALL parity", StatementOptions::default())
        .await
        .expect("prepare call");
    assert_eq!(
        callable.execute_prepared_update().await.expect("execute"),
        1
    );
    assert_eq!(callable.was_null(), Ok(false));
    assert!(matches!(
        callable.out_value(ParameterIndex::new(1).expect("index")),
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    let before = pool.stat_value_and_reset();
    assert_eq!(before.error_count, 0);

    fail_was_null.store(true, Ordering::Release);
    let error = callable.was_null().expect_err("injected SQL exception");
    let exception = error.sql_exception().expect("ordinary SQL exception");
    assert_eq!(exception.class_name(), "java.sql.SQLException");
    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    let after = pool.stat_value_and_reset();
    assert_eq!(after.error_count, 1);
    assert_eq!(after.execute_count, 0);

    Statement::close(callable.as_mut())
        .await
        .expect("callable close");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().idle_count, 1);
    pool.close().await;
}
