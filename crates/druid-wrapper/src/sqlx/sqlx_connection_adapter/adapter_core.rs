use super::*;

impl SqlxConnectionAdapter {
    /// 直接连接数据库并创建 Adapter。
    ///
    /// 参数 `url` 为 `SQLx` 数据库 URL；返回未池化的物理连接 Adapter。
    pub async fn connect(url: &str) -> Result<Self, RdbcError> {
        let connection = if url.starts_with("sqlite:") {
            SqlxConnectionBackend::Sqlite(
                SqliteConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else if url.starts_with("mysql:") {
            SqlxConnectionBackend::MySql(
                MySqlConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            SqlxConnectionBackend::PostgreSql(
                PgConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else {
            sqlx::any::install_default_drivers();
            SqlxConnectionBackend::Any(
                AnyConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        };
        Ok(Self {
            connection: Some(connection),
            url: url.to_owned(),
            savepoint_sequence: 0,
            auto_commit: true,
            read_only: false,
            discarded: false,
            actor_discarded: None,
            standard_actor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    pub(crate) fn attach_actor_discard_flag(&mut self, flag: Arc<AtomicBool>) {
        self.actor_discarded = Some(flag);
    }

    pub(crate) fn take_runtime_trace(&mut self) -> Option<RuntimeTraceEmitter> {
        self.runtime_trace.take()
    }

    pub(crate) fn validate_savepoint_name(name: &str) -> Result<(), RdbcError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(RdbcError::InvalidArgument(
                "savepoint name must contain only ASCII letters, digits, or '_'".to_owned(),
            ))
        }
    }

    /// 把当前 raw adapter 转成 canonical owned actor，并在 connection handle 中保留 clone。
    pub(crate) fn standard_actor_core(&mut self) -> Result<Arc<SqlxConnectionActor>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return Ok(Arc::clone(actor));
        }
        if self.connection.is_none() || self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let detached = Self {
            connection: None,
            url: self.url.clone(),
            savepoint_sequence: self.savepoint_sequence,
            auto_commit: self.auto_commit,
            read_only: self.read_only,
            discarded: self.discarded,
            actor_discarded: None,
            standard_actor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, detached);
        let actor = SqlxConnectionActor::new(owned);
        self.standard_actor = Some(Arc::clone(&actor));
        Ok(actor)
    }

    pub(crate) fn standard_auto_commit_core(&self) -> bool {
        self.auto_commit
    }

    pub(crate) fn standard_read_only_core(&self) -> bool {
        self.read_only
    }

    pub(crate) const fn standard_savepoint_sequence_core(&self) -> u64 {
        self.savepoint_sequence
    }

    pub(crate) fn standard_url_core(&self) -> &str {
        &self.url
    }

    pub(crate) fn standard_product_name_core(&self) -> Result<String, RdbcError> {
        match self.backend_key() {
            "sqlite" => Ok("SQLite".to_owned()),
            "mysql" => Ok("MySQL".to_owned()),
            "postgresql" => Ok("PostgreSQL".to_owned()),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_database_metadata_product_name",
            }),
        }
    }

    pub(crate) async fn standard_product_version_core(&mut self) -> Result<String, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Sqlite(connection) => {
                sqlx::query_scalar::<Sqlite, String>("select sqlite_version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::MySql(connection) => {
                sqlx::query_scalar::<MySql, String>("select version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query_scalar::<Postgres, String>("show server_version")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Any(connection) => {
                sqlx::query_scalar::<Any, String>("select version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }
    }

    pub(crate) async fn standard_user_name_core(&mut self) -> Result<Option<String>, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Sqlite(_) => Ok(None),
            SqlxConnectionBackend::MySql(connection) => {
                sqlx::query_scalar::<MySql, String>("select current_user()")
                    .fetch_one(connection)
                    .await
                    .map(Some)
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query_scalar::<Postgres, String>("select current_user")
                    .fetch_one(connection)
                    .await
                    .map(Some)
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Any(_) => Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_database_metadata_user_name",
            }),
        }
    }

    pub(crate) async fn standard_read_only_metadata_core(&mut self) -> Result<bool, RdbcError> {
        if self.backend_key() == "sqlite" {
            if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
                return sqlx::query_scalar::<Sqlite, i64>("PRAGMA query_only")
                    .fetch_one(connection)
                    .await
                    .map(|value| value != 0)
                    .map_err(Self::driver_error);
            }
        }
        Ok(self.read_only)
    }

    pub(crate) async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        match self.connection.take() {
            Some(SqlxConnectionBackend::Any(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::MySql(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::PostgreSql(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::Sqlite(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            None => {}
        }
        Ok(())
    }

    pub(crate) async fn ping_connection_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return actor.ping().await;
        }
        self.ping_raw_core().await
    }

    pub(crate) async fn ping_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::PostgreSql(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::Sqlite(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
        }
    }

    pub(crate) async fn begin_raw_core(&mut self) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::DriverError(
                "SQLx transaction is already active".to_owned(),
            ));
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                AnyTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::MySql(connection) => {
                MySqlTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = false;
        Ok(())
    }

    pub(crate) async fn commit_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::commit(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => MySqlTransactionManager::commit(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::commit(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::commit(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = true;
        Ok(())
    }

    pub(crate) async fn rollback_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::rollback(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => {
                MySqlTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = true;
        Ok(())
    }

    pub(crate) async fn set_auto_commit_core(&mut self, enabled: bool) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return actor.set_auto_commit(enabled).await;
        }
        match (self.auto_commit, enabled) {
            (true, false) => self.begin_raw_core().await,
            (false, true) => self.commit_raw_core().await,
            _ => Ok(()),
        }
    }

    pub(crate) async fn commit_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.commit().await
        } else {
            self.commit_raw_core().await
        }
    }

    pub(crate) async fn rollback_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.rollback().await
        } else {
            self.rollback_raw_core().await
        }
    }

    pub(crate) async fn execute_control_statement(&mut self, sql: &str) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::MySql(connection) => {
                // MySQL 的 SAVEPOINT 等控制命令不支持 prepared protocol。
                // 此入口只接收已校验的无绑定控制 SQL，使用原连接的 text protocol。
                connection.execute(sql).await.map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn set_read_only_raw_core(
        &mut self,
        read_only: bool,
    ) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "read_only cannot change while a transaction is active".to_owned(),
            ));
        }
        self.connection_mut()?;
        let backend = self.backend_key();
        let Some(sql) = read_only_session_sql(backend, read_only) else {
            if read_only {
                return Err(RdbcError::UnsupportedOperation {
                    operation: if backend == "sqlite" {
                        "sqlx_sqlite_read_only_transaction"
                    } else {
                        "sqlx_driver_read_only_transaction"
                    },
                });
            }
            self.read_only = false;
            return Ok(());
        };
        self.execute_control_statement(sql).await?;
        self.read_only = read_only;
        Ok(())
    }

    pub(crate) async fn standard_control_core(&mut self, sql: &str) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.execute_control_statement(sql).await
        } else {
            self.execute_control_statement(sql).await
        }
    }

    pub(crate) fn next_savepoint_id_core(&mut self) -> u64 {
        if let Some(actor) = &self.standard_actor {
            actor.next_savepoint_id()
        } else {
            self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
            self.savepoint_sequence
        }
    }

    pub(crate) fn standard_backend_key_core(&self) -> &'static str {
        self.standard_actor
            .as_ref()
            .map_or_else(|| self.backend_key(), |actor| actor.backend_key())
    }

    pub(crate) fn connection_mut(&mut self) -> Result<&mut SqlxConnectionBackend, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    /// 让 SQLite worker 在执行被取消或截止时间到达时自行中断当前 VM。
    /// Tokio future 被取消时，底层 SQLite worker 仍可能继续运行；因此必须把截止时间
    /// 安装到物理连接，而不能只在调用方等待 future 的地方计时。
    pub(crate) async fn install_sqlite_progress_control(
        &mut self,
        deadline: Option<Instant>,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), RdbcError> {
        if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
            let mut handle = connection.lock_handle().await.map_err(Self::driver_error)?;
            handle.set_progress_handler(1_000, move || {
                !interrupted.load(std::sync::atomic::Ordering::Acquire)
                    && deadline.is_none_or(|deadline| Instant::now() < deadline)
            });
        }
        Ok(())
    }

    /// 等待 SQLite worker 释放物理句柄，并清除本次执行的中断回调。
    pub(crate) async fn clear_sqlite_progress_deadline(&mut self) -> Result<(), RdbcError> {
        if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
            connection
                .lock_handle()
                .await
                .map_err(Self::driver_error)?
                .remove_progress_handler();
        }
        Ok(())
    }

    /// 无法确认 worker 已释放时，禁止将物理连接重新借给池。
    pub(crate) fn discard_after_unconfirmed_interrupt(&mut self) {
        self.discarded = true;
        if let Some(flag) = &self.actor_discarded {
            flag.store(true, Ordering::Release);
        }
    }

    pub(crate) fn backend_key(&self) -> &'static str {
        match self.connection {
            Some(SqlxConnectionBackend::Any(_)) => "any",
            Some(SqlxConnectionBackend::MySql(_)) => "mysql",
            Some(SqlxConnectionBackend::PostgreSql(_)) => "postgresql",
            Some(SqlxConnectionBackend::Sqlite(_)) => "sqlite",
            None => "closed",
        }
    }

    pub(crate) fn driver_error(error: sqlx::Error) -> RdbcError {
        match error {
            sqlx::Error::Database(database_error) => {
                let sql_state = database_error.code().map(std::borrow::Cow::into_owned);
                let error_code = sql_state
                    .as_deref()
                    .and_then(|code| code.parse::<i32>().ok())
                    .unwrap_or_default();
                RdbcError::SqlException(Box::new(
                    druid_rdbc::error::SqlException::new(
                        error_code,
                        sql_state,
                        Some(database_error.message().to_string()),
                    )
                    .with_class_name("sqlx::error::DatabaseError"),
                ))
            }
            sqlx::Error::Io(error) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("std::io::Error")
                .recoverable(),
            )),
            sqlx::Error::Tls(error) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("sqlx::error::TlsError")
                .recoverable(),
            )),
            sqlx::Error::Protocol(message) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(0, Some("08006".to_owned()), Some(message))
                    .with_class_name("sqlx::error::ProtocolError")
                    .recoverable(),
            )),
            error @ (sqlx::Error::PoolTimedOut
            | sqlx::Error::PoolClosed
            | sqlx::Error::WorkerCrashed) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("sqlx::Error")
                .recoverable(),
            )),
            error => RdbcError::DriverError(error.to_string()),
        }
    }

    pub(crate) fn bind_any_values(
        sql: &str,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'_, Any, sqlx::any::AnyArguments<'_>>, RdbcError> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
                Value::Decimal(_) => return Err(any_bind_unsupported("decimal")),
                Value::Date(_) => return Err(any_bind_unsupported("date")),
                Value::Time(_) => return Err(any_bind_unsupported("time")),
                Value::Timestamp(_) => return Err(any_bind_unsupported("timestamp")),
            };
        }
        Ok(query)
    }

    pub(crate) fn bind_sqlite_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, Sqlite, SqliteArguments<'_>> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                // SQLite 没有 DECIMAL storage class。绑定阶段使用十进制文本，
                // 但 NUMERIC affinity 仍可能在存储时转为 INTEGER/REAL；读取端
                // 必须按真实 runtime storage class 上报，不能伪造 Decimal。
                Value::Decimal(value) => query.bind(value.to_string()),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn bind_mysql_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, MySql, MySqlArguments> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn bind_postgresql_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, Postgres, PgArguments> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn bind_any_prepared_values<'query>(
        statement: &'query sqlx::any::AnyStatement<'query>,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>>, RdbcError> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
                Value::Decimal(_) => return Err(any_bind_unsupported("decimal")),
                Value::Date(_) => return Err(any_bind_unsupported("date")),
                Value::Time(_) => return Err(any_bind_unsupported("time")),
                Value::Timestamp(_) => return Err(any_bind_unsupported("timestamp")),
            };
        }
        Ok(query)
    }

    pub(crate) fn bind_sqlite_prepared_values<'query>(
        statement: &'query sqlx::sqlite::SqliteStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Sqlite, SqliteArguments<'query>> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value.to_string()),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn bind_mysql_prepared_values<'query>(
        statement: &'query sqlx::mysql::MySqlStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, MySql, MySqlArguments> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn bind_postgresql_prepared_values<'query>(
        statement: &'query sqlx::postgres::PgStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Postgres, PgArguments> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    pub(crate) fn decode_any_row(row: AnyRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }

            let value = match column.type_info().kind() {
                AnyTypeInfoKind::Null => Value::Null,
                AnyTypeInfoKind::Bool => {
                    Value::Bool(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::SmallInt => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                AnyTypeInfoKind::Integer => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                AnyTypeInfoKind::BigInt => {
                    Value::Int(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Real => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                AnyTypeInfoKind::Double => {
                    Value::Float(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Text => {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Blob => {
                    Value::Bytes(row.try_get(index).map_err(Self::driver_error)?)
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }
}
