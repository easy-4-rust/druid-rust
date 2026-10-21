//! `SQLx` canonical 连接适配器。

use super::sqlx_connection_actor::SqlxConnectionActor;
use super::sqlx_prepared_statement::SqlxPreparedStatement;
use crate::standard_adapter_executor::AdapterUpdateResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, ResultSetColumnType};
use druid_rdbc::types::{Row, Types, Value};
use sqlx::any::{AnyRow, AnyTransactionManager, AnyTypeInfoKind};
use sqlx::mysql::{MySqlArguments, MySqlConnection, MySqlRow, MySqlTransactionManager};
use sqlx::postgres::{PgArguments, PgConnection, PgRow, PgTransactionManager};
use sqlx::sqlite::{SqliteArguments, SqliteRow, SqliteTransactionManager};
use sqlx::{
    Any, AnyConnection, Column, Connection as SqlxConnection, Executor, MySql, Postgres,
    Row as SqlxRow, Sqlite, SqliteConnection, Statement, TransactionManager, TypeInfo, ValueRef,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod adapter_core;
mod adapter_queries;

type SqlxExecResult = AdapterUpdateResult;

fn any_bind_unsupported(value_type: &'static str) -> RdbcError {
    RdbcError::UnsupportedOperation {
        operation: match value_type {
            "decimal" => "sqlx_any_bind_decimal",
            "date" => "sqlx_any_bind_date",
            "time" => "sqlx_any_bind_time",
            "timestamp" => "sqlx_any_bind_timestamp",
            _ => "sqlx_any_bind_strong_value",
        },
    }
}

fn read_only_session_sql(backend: &str, read_only: bool) -> Option<&'static str> {
    match (backend, read_only) {
        ("postgresql", true) => Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"),
        ("postgresql", false) => Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE"),
        ("mysql", true) => Some("SET SESSION TRANSACTION READ ONLY"),
        ("mysql", false) => Some("SET SESSION TRANSACTION READ WRITE"),
        _ => None,
    }
}

pub(crate) enum SqlxConnectionBackend {
    Any(AnyConnection),
    MySql(MySqlConnection),
    PostgreSql(PgConnection),
    Sqlite(SqliteConnection),
}

/// `SQLx` 物理连接适配器。
///
/// 对应 Java 平台依赖: `java.sql.Connection` 的驱动实现。
/// 本对象只包装一个 `SQLx` Connection，不包含 `SQLx` Pool、bb8 或 deadpool，
/// 因而不会形成 pool-in-pool。SQLite、MySQL、PostgreSQL 使用各自原生连接，
/// 以保留 Decimal 和日期时间等强类型；未知 `SQLx` scheme 才回退到 `Any`。
pub struct SqlxConnectionAdapter {
    connection: Option<SqlxConnectionBackend>,
    url: String,
    savepoint_sequence: u64,
    auto_commit: bool,
    read_only: bool,
    discarded: bool,
    actor_discarded: Option<Arc<AtomicBool>>,
    standard_actor: Option<Arc<SqlxConnectionActor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

druid_rdbc::impl_standard_wrapper!(SqlxConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for SqlxConnectionAdapter {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = if let Some(actor) = &self.standard_actor {
            actor
                .execute_update_result(
                    sql,
                    params,
                    0,
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                )
                .await?
        } else {
            self.execute_update_core(sql, params).await?
        };
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.fetch_rows(sql, params).await
        } else {
            self.fetch_rows_with_labels(sql, params)
                .await
                .map(|(rows, _)| rows)
        }
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        let actor = self.standard_actor_core()?;
        Ok(Box::new(SqlxPreparedStatement::owned_statement(
            String::new(),
            actor,
            options,
            0,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        self.prepare_statement_with_generated_keys(
            sql,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
    }

    /// SQLite auto 准备将请求存入 owned statement；未验证的模式在建句柄前拒绝。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        match &generated_keys {
            druid_rdbc::statement::GeneratedKeys::None => {}
            druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys
                if self.standard_backend_key_core() == "sqlite" => {}
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "connection_prepare_statement_generated_keys",
                ));
            }
        }
        let actor = self.standard_actor_core()?;
        let parameter_count = actor.parameter_count(sql).await?;
        let result_set_meta_data = druid_rdbc::metadata::ResultSetMetaDataSnapshot::new(
            actor.describe_columns(sql).await?,
        );
        Ok(Box::new(
            SqlxPreparedStatement::owned_prepared_statement(
                sql.to_owned(),
                actor,
                options,
                parameter_count,
                result_set_meta_data,
            )
            .with_generated_keys_request(generated_keys),
        ))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        Ok(Box::new(
            super::sqlx_database_meta_data::SqlxDatabaseMetaData::owned(
                self.standard_actor_core()?,
            ),
        ))
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.commit_core().await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.rollback_core().await
    }

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.standard_control_core(&format!("ROLLBACK TO SAVEPOINT {name}"))
            .await
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let id = self.next_savepoint_id_core();
        self.standard_control_core(&format!("SAVEPOINT druid_sp_{id}"))
            .await?;
        Ok(druid_rdbc::types::Savepoint { id, name: None })
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        Self::validate_savepoint_name(name)?;
        let id = self.next_savepoint_id_core();
        self.standard_control_core(&format!("SAVEPOINT {name}"))
            .await?;
        Ok(druid_rdbc::types::Savepoint {
            id,
            name: Some(name.to_owned()),
        })
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.standard_control_core(&format!("RELEASE SAVEPOINT {name}"))
            .await
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.set_auto_commit_core(enabled).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_actor.as_ref().map_or_else(
            || self.standard_auto_commit_core(),
            |actor| actor.auto_commit(),
        ))
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self
            .standard_actor
            .as_ref()
            .map_or_else(|| self.standard_read_only_core(), |actor| actor.read_only()))
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.set_read_only(read_only).await
        } else {
            self.set_read_only_raw_core(read_only).await
        }
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        // 仅原生后端拥有已验证的事务和保存点合同；Any 回退不继承其能力声明。
        let verified_transactions = matches!(
            self.standard_backend_key_core(),
            "sqlite" | "mysql" | "postgresql"
        );
        druid_rdbc::extensions::ConnectionCapabilities {
            transactions: verified_transactions,
            savepoints: verified_transactions,
            catalogs: false,
            schemas: false,
            callable_statements: false,
        }
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        match self.ping_connection_core().await {
            Ok(()) => Ok(true),
            Err(RdbcError::ConnectionDiscarded) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.ping_connection_core().await
    }

    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.warnings()
        } else {
            self.connection_mut()?;
            Ok(None)
        }
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.clear_warnings()
        } else {
            self.connection_mut()?;
            Ok(())
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self
            .standard_actor
            .as_ref()
            .map_or_else(|| self.connection.is_none(), |actor| actor.is_closed()))
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok(match self.standard_backend_key_core() {
            "mysql" => "MySQL",
            "postgresql" => "PostgreSQL",
            "sqlite" => "SQLite",
            "any" => "SQLx Any",
            _ => "sqlx-closed",
        }
        .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{read_only_session_sql, SqlxConnectionAdapter, SqlxConnectionBackend};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::StatementOptions;
    use druid_rdbc::types::{RdbcObject, Value};
    use sqlx::any::AnyConnectOptions;
    use sqlx::ConnectOptions;

    fn assert_feature_not_supported<T>(result: Result<T, RdbcError>, operation: &str) {
        match result {
            Err(RdbcError::SqlException(error)) => {
                assert_eq!(error.sql_state(), Some("0A000"), "{operation}");
            }
            Err(error) => panic!("{operation} returned the wrong error: {error:?}"),
            Ok(_) => panic!("{operation} unexpectedly succeeded"),
        }
    }

    #[test]
    fn read_only_session_sql_is_backend_specific() {
        assert_eq!(
            read_only_session_sql("postgresql", true),
            Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
        );
        assert_eq!(
            read_only_session_sql("mysql", false),
            Some("SET SESSION TRANSACTION READ WRITE")
        );
        assert_eq!(read_only_session_sql("sqlite", true), None);
        assert_eq!(read_only_session_sql("any", true), None);
    }

    #[tokio::test]
    async fn any_backend_capabilities_and_cancellation_are_conservative() {
        sqlx::any::install_default_drivers();
        let options = "sqlite::memory:"
            .parse::<AnyConnectOptions>()
            .expect("Any connection URL should parse");
        let connection = options
            .connect()
            .await
            .expect("SQLx Any SQLite driver should open for fallback capability testing");
        let mut adapter = SqlxConnectionAdapter {
            connection: Some(SqlxConnectionBackend::Any(connection)),
            url: "sqlite::memory:".to_owned(),
            savepoint_sequence: 0,
            auto_commit: true,
            read_only: false,
            discarded: false,
            actor_discarded: None,
            standard_actor: None,
            runtime_trace: None,
        };
        let conservative = druid_rdbc::extensions::ConnectionCapabilities::default();
        assert_eq!(adapter.capabilities(), conservative);
        let mut statement = adapter
            .create_statement(StatementOptions::default())
            .await
            .expect("Any-backed statement should be created");
        assert_eq!(adapter.capabilities(), conservative);
        let error = match statement.cancellation_control() {
            Ok(_) => panic!("Any fallback must not advertise SQLite-native cancellation"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RdbcError::SqlException(ref sql_error) if sql_error.sql_state() == Some("0A000")
        ));
        statement.close().await.unwrap();
        adapter.close().await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_live_resources_capabilities_and_async_result_set_are_truthful() {
        let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
            .await
            .expect("real SQLx SQLite connection should open");

        assert_eq!(
            connection.capabilities(),
            druid_rdbc::extensions::ConnectionCapabilities {
                transactions: true,
                savepoints: true,
                catalogs: false,
                schemas: false,
                callable_statements: false,
            }
        );

        assert_feature_not_supported(
            connection
                .create_array_of(Some("INTEGER"), Some(Vec::new()))
                .await,
            "Connection#createArrayOf",
        );
        assert_feature_not_supported(connection.create_blob().await, "Connection#createBlob");
        assert_feature_not_supported(connection.create_clob().await, "Connection#createClob");
        assert_feature_not_supported(connection.create_n_clob().await, "Connection#createNClob");
        assert_feature_not_supported(connection.create_sql_xml().await, "Connection#createSQLXML");
        assert_feature_not_supported(
            connection
                .create_struct(Some("address"), Some(Vec::<RdbcObject>::new()))
                .await,
            "Connection#createStruct",
        );

        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .expect("standard statement should be created");
        let mut result_set = statement
            .execute_query("SELECT 10 UNION ALL SELECT 20 UNION ALL SELECT 30")
            .await
            .expect("SQLite should execute the real multi-row query");
        let column = ColumnIndex::new(1).expect("one-based column index should be valid");
        for expected in [10, 20, 30] {
            assert!(result_set.next().await.expect("row fetch should succeed"));
            assert_eq!(
                result_set.value(column).expect("current row exists"),
                Value::Int(expected)
            );
        }
        assert!(!result_set.next().await.expect("EOF should be observable"));
        result_set
            .close()
            .await
            .expect("result set should close explicitly");
        statement.close().await.expect("statement should close");
        connection
            .close()
            .await
            .expect("SQLite connection should close");
    }
}
