//! `java.sql.Statement` 的 JDBC 常量精确值与 generated-keys 重载转发契约。
//!
//! 常量部分逐个断言 `DruidPooledStatement` 上的 7 个 JDBC 兼容常量与 JDK8 精确一致；
//! keys 重载部分通过记录型 Statement 句柄，断言 `executeUpdate(String, int[])` 与
//! `executeUpdate(String, String[])` 分别以 `GeneratedKeys::ColumnIndexes` 与
//! `GeneratedKeys::ColumnNames` 的形式把 SQL 与 keys 转发到驱动层。

use druid::core::{DruidPooledConnection, DruidPooledStatement};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, Statement, StatementOptions};
use std::sync::{Arc, Mutex};

/// 记录一次 `execute_update` 转发的 SQL 与 generated-keys 描述符。
#[derive(Debug, Clone, PartialEq)]
struct UpdateCall {
    sql: String,
    keys: GeneratedKeys,
}

/// 记录型状态容器，保存全部 update 转发记录。
#[derive(Debug, Default)]
struct State {
    updates: Mutex<Vec<UpdateCall>>,
}

/// 记录型 Statement：只实现 `execute_update`，捕获 SQL 与 keys 转发。
struct RecordingStatement {
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingStatement => Statement);

#[async_trait::async_trait]
impl Statement for RecordingStatement {
    async fn execute_update(&mut self, sql: &str, keys: GeneratedKeys) -> Result<i64, RdbcError> {
        self.state
            .updates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(UpdateCall {
                sql: sql.to_owned(),
                keys,
            });
        Ok(1)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

/// 记录型 Connection：为每次 `create_statement` 提供记录型 Statement。
struct RecordingConnection {
    state: Arc<State>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(RecordingStatement {
            state: Arc::clone(&self.state),
        }))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("recording-statement-keys".to_owned())
    }
}

/// 记录型 ConnectionFactory，向池提供记录型 Connection。
struct RecordingFactory {
    state: Arc<State>,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            state: Arc::clone(&self.state),
        }))
    }
}

/// 建立一个池化 `java.sql.Statement` 句柄，返回记录状态、租约与句柄。
///
/// 连接租约必须随句柄一起存活，否则池化语句会在租约失效后拒绝执行。
async fn recording_statement() -> (Arc<State>, DruidPooledConnection, Box<dyn Statement>) {
    let state = Arc::new(State::default());
    let pool = DruidPool::builder()
        .name("statement-keys")
        .driver_name("recording-statement-keys")
        .factory(Arc::new(RecordingFactory {
            state: Arc::clone(&state),
        }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("recording pool");
    let mut connection = pool.get().await.expect("pooled connection");
    let statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("pooled statement");
    (state, connection, statement)
}

/// 读取记录型 Statement 捕获的全部 update 转发。
fn updates(state: &State) -> Vec<UpdateCall> {
    state
        .updates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// `java.sql.Statement` 的 7 个 JDBC 兼容常量必须与 JDK8 精确一致。
#[test]
fn statement_constants_match_jdk8_exact_values() {
    assert_eq!(DruidPooledStatement::CLOSE_ALL_RESULTS, 3);
    assert_eq!(DruidPooledStatement::CLOSE_CURRENT_RESULT, 1);
    assert_eq!(DruidPooledStatement::EXECUTE_FAILED, -3);
    assert_eq!(DruidPooledStatement::KEEP_CURRENT_RESULT, 2);
    assert_eq!(DruidPooledStatement::NO_GENERATED_KEYS, 2);
    assert_eq!(DruidPooledStatement::RETURN_GENERATED_KEYS, 1);
    assert_eq!(DruidPooledStatement::SUCCESS_NO_INFO, -2);
}

/// `executeUpdate(String, int[])` 必须按 1-based 列下标转发 SQL 与 generated keys。
#[tokio::test]
async fn statement_execute_update_forwards_column_index_generated_keys() {
    let (state, _connection, mut statement) = recording_statement().await;
    let sql = "UPDATE t SET v = 1";
    let keys = GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]);

    assert_eq!(
        statement.execute_update(sql, keys.clone()).await.unwrap(),
        1
    );

    assert_eq!(
        updates(&state),
        vec![UpdateCall {
            sql: sql.to_owned(),
            keys,
        }]
    );
}

/// `executeUpdate(String, String[])` 必须按列名转发 SQL 与 generated keys。
#[tokio::test]
async fn statement_execute_update_forwards_column_name_generated_keys() {
    let (state, _connection, mut statement) = recording_statement().await;
    let sql = "UPDATE t SET v = 2";
    let keys = GeneratedKeys::ColumnNames(vec!["id".to_owned()]);

    assert_eq!(
        statement.execute_update(sql, keys.clone()).await.unwrap(),
        1
    );

    assert_eq!(
        updates(&state),
        vec![UpdateCall {
            sql: sql.to_owned(),
            keys,
        }]
    );
}
