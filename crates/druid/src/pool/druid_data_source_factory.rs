use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::core::{ConfigFilter, DruidError, PhysicalConnectionFactory};
use crate::sql::{JdbcUtils, WallConfig};
use crate::toasty::ToastyConnectionFactory;

use super::{DruidDataSource, DruidPoolBuilder};

/// 从 Java Druid 属性创建 canonical 数据源。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidDataSourceFactory`。默认入口使用
/// Toasty 创建未池化物理连接；SQLx/RBDC 等扩展通过
/// [`Self::create_data_source_with_factory`] 注入自己的最小 SPI factory。
pub struct DruidDataSourceFactory;

impl DruidDataSourceFactory {
    /// 使用内置 Toasty 驱动创建数据源。
    ///
    /// # Errors
    ///
    /// 缺少 URL、属性格式错误、使用独立 username/password 或 Toasty 连接失败
    /// 时返回结构化错误。独立凭据必须交给能原生表达它们的扩展 factory，不能
    /// 无声丢弃。
    pub async fn create_data_source(
        properties: &HashMap<String, String>,
    ) -> Result<DruidDataSource, DruidError> {
        let properties = Self::resolve_config_properties(properties).await?;
        Self::create_data_source_resolved(&properties).await
    }

    /// 在构造具体驱动 factory 前执行 Java `ConfigFilter` 初始化语义。
    ///
    /// SQLx/RBDC 等扩展应先调用此方法，再用返回的 URL、用户名与密码构造
    /// `PhysicalConnectionFactory`，从而避免先建驱动、后下载配置的错误顺序。
    pub async fn resolve_config_properties(
        properties: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, DruidError> {
        if ConfigFilter::is_enabled(properties) {
            ConfigFilter::new().resolve_properties(properties).await
        } else {
            Ok(properties.clone())
        }
    }

    async fn create_data_source_resolved(
        properties: &HashMap<String, String>,
    ) -> Result<DruidDataSource, DruidError> {
        if properties.contains_key(Self::PROP_USERNAME)
            || properties.contains_key(Self::PROP_PASSWORD)
        {
            return Err(DruidError::InvalidArgument(
                "Toasty default datasource requires credentials in the URL; use an extension PhysicalConnectionFactory for separate username/password".to_owned(),
            ));
        }
        let configured_url = required(properties, Self::PROP_URL)?;
        let url = JdbcUtils::to_rust_url(configured_url).ok_or_else(|| {
            DruidError::InvalidArgument(format!(
                "Toasty does not support JDBC URL `{configured_url}`; select a druid-wrapper PhysicalConnectionFactory adapter"
            ))
        })?;
        let factory = ToastyConnectionFactory::new(url.as_ref()).await?;
        let driver_name = properties
            .get(Self::PROP_DRIVER_CLASS_NAME)
            .cloned()
            .unwrap_or_else(|| factory.driver_name().to_owned());
        if let (Some(limit), Some(max_active)) = (
            factory.max_connections(),
            optional_usize(properties, Self::PROP_MAX_ACTIVE)?,
        ) {
            if max_active > limit {
                return Err(DruidError::InvalidArgument(format!(
                    "maxActive {max_active} exceeds driver connection limit {limit}"
                )));
            }
        }
        Self::create_data_source_with_factory_resolved(properties, Arc::new(factory), driver_name)
            .await
    }

    /// 使用外部未池化物理连接 factory 创建数据源。
    pub async fn create_data_source_with_factory(
        properties: &HashMap<String, String>,
        factory: Arc<dyn PhysicalConnectionFactory>,
        default_driver_name: impl Into<String>,
    ) -> Result<DruidDataSource, DruidError> {
        let properties = Self::resolve_config_properties(properties).await?;
        Self::create_data_source_with_factory_resolved(&properties, factory, default_driver_name)
            .await
    }

    async fn create_data_source_with_factory_resolved(
        properties: &HashMap<String, String>,
        factory: Arc<dyn PhysicalConnectionFactory>,
        default_driver_name: impl Into<String>,
    ) -> Result<DruidDataSource, DruidError> {
        let driver_name = properties
            .get(Self::PROP_DRIVER_CLASS_NAME)
            .cloned()
            .unwrap_or_else(|| default_driver_name.into());
        let connection_properties = physical_connection_properties(properties);
        let mut builder = DruidPoolBuilder::new()
            .factory(factory)
            .driver_name(&driver_name)
            .connection_properties(connection_properties);
        if let Some(url) = properties.get(Self::PROP_URL) {
            builder = builder.url(url).raw_url(url);
        }

        // Java DruidDataSource#init 会在未显式设置时从 jdbcUrl 推断 dbType。
        // Rust 同时接受 JDBC URL 与 sqlx/toasty URL，确保 Wall、校验器和
        // ExceptionSorter 使用同一个数据库身份。
        let db_type = properties.get(Self::PROP_DB_TYPE).cloned().or_else(|| {
            JdbcUtils::infer_db_type(
                properties.get(Self::PROP_URL).map(String::as_str),
                Some(&driver_name),
            )
            .map(|db_type| db_type.as_str().to_owned())
        });
        if let Some(db_type) = db_type {
            builder = builder.db_type_name(db_type);
        }
        if let Some(wall_config) = wall_config_from_properties(properties)? {
            builder = builder.wall_config(wall_config);
        }

        if let Some(value) = properties.get(Self::PROP_NAME) {
            builder = builder.name(value);
        }
        if let Some(value) = properties.get(Self::PROP_DEFAULT_AUTO_COMMIT) {
            builder =
                builder.default_auto_commit(parse_bool(Self::PROP_DEFAULT_AUTO_COMMIT, value)?);
        }
        if let Some(value) = properties.get(Self::PROP_DEFAULT_READ_ONLY) {
            builder = builder.default_read_only(parse_bool(Self::PROP_DEFAULT_READ_ONLY, value)?);
        }
        if let Some(value) = properties.get(Self::PROP_DEFAULT_TRANSACTION_ISOLATION) {
            if let Some(level) = parse_transaction_isolation(value)? {
                builder = builder.default_transaction_isolation(level);
            }
        }
        if let Some(value) = properties.get(Self::PROP_DEFAULT_CATALOG) {
            builder = builder.default_catalog(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_INIT_VARIANTS)? {
            builder = builder.init_variants(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_INIT_GLOBAL_VARIANTS)? {
            builder = builder.init_global_variants(value);
        }
        if let Some(value) = properties.get(Self::PROP_INIT_CONNECTION_SQLS) {
            builder = builder.connection_init_sqls(value.split(';'));
        }
        if let Some(value) = optional_usize(properties, Self::PROP_MAX_ACTIVE)? {
            builder = builder.max_open(value);
        }
        if let Some(value) = optional_usize(properties, Self::PROP_MAX_IDLE)? {
            builder = builder.max_idle(value);
        }
        if let Some(value) = optional_usize(properties, Self::PROP_MIN_IDLE)? {
            builder = builder.min_idle(value);
        }
        if let Some(value) = optional_usize(properties, Self::PROP_INITIAL_SIZE)? {
            builder = builder.initial_size(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_ASYNC_INIT)? {
            builder = builder.async_init(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_INIT_EXCEPTION_THROW)? {
            builder = builder.init_exception_throw(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_MAX_WAIT)? {
            builder = builder.acquire_timeout(if value < 0 {
                Duration::MAX
            } else {
                Duration::from_millis(value as u64)
            });
        }
        if let Some(value) = optional_i64(properties, Self::PROP_NOT_FULL_TIMEOUT_RETRY_COUNT)? {
            builder = builder.not_full_timeout_retry_count(i32_property(
                Self::PROP_NOT_FULL_TIMEOUT_RETRY_COUNT,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_MAX_WAIT_THREAD_COUNT)? {
            builder = builder.max_wait_thread_count(if value <= 0 {
                None
            } else {
                Some(usize::try_from(value).map_err(|_| {
                    DruidError::InvalidArgument(format!(
                        "{} is out of range: {value}",
                        Self::PROP_MAX_WAIT_THREAD_COUNT
                    ))
                })?)
            });
        }
        if let Some(value) = optional_i64(properties, Self::PROP_CONNECTION_ERROR_RETRY_ATTEMPTS)? {
            builder = builder.connection_error_retry_attempts(if value <= 0 {
                0
            } else {
                usize::try_from(value).map_err(|_| {
                    DruidError::InvalidArgument(format!(
                        "{} is out of range: {value}",
                        Self::PROP_CONNECTION_ERROR_RETRY_ATTEMPTS
                    ))
                })?
            });
        }
        if let Some(value) = optional_bool(properties, Self::PROP_BREAK_AFTER_ACQUIRE_FAILURE)? {
            builder = builder.break_after_acquire_failure(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_TIME_BETWEEN_CONNECT_ERROR_MILLIS)?
        {
            builder = builder.time_between_connect_error(non_negative_duration(
                Self::PROP_TIME_BETWEEN_CONNECT_ERROR_MILLIS,
                value,
            )?);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_FAIL_FAST)? {
            builder = builder.fail_fast(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_ON_FATAL_ERROR_MAX_ACTIVE)? {
            builder = builder.on_fatal_error_max_active(i32_property(
                Self::PROP_ON_FATAL_ERROR_MAX_ACTIVE,
                value,
            )?);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_TEST_ON_BORROW)? {
            builder = builder.test_on_borrow(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_TEST_ON_RETURN)? {
            builder = builder.test_on_return(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_TEST_WHILE_IDLE)? {
            builder = builder.test_while_idle(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_TIME_BETWEEN_EVICTION_RUNS_MILLIS)?
        {
            builder = builder.time_between_eviction_runs(non_negative_duration(
                Self::PROP_TIME_BETWEEN_EVICTION_RUNS_MILLIS,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_MIN_EVICTABLE_IDLE_TIME_MILLIS)? {
            builder = builder.idle_timeout(non_negative_duration(
                Self::PROP_MIN_EVICTABLE_IDLE_TIME_MILLIS,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_MAX_EVICTABLE_IDLE_TIME_MILLIS)? {
            builder = builder.max_evictable_idle_time(non_negative_duration(
                Self::PROP_MAX_EVICTABLE_IDLE_TIME_MILLIS,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_PHY_TIMEOUT_MILLIS)?
            .or(optional_i64(properties, "druid.phyTimeoutMillis")?)
        {
            builder = builder.physical_connection_timeout(non_negative_duration(
                Self::PROP_PHY_TIMEOUT_MILLIS,
                value,
            )?);
        }
        if let Some(value) = properties.get(Self::PROP_VALIDATION_QUERY) {
            builder = builder.validation_query(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_VALIDATION_QUERY_TIMEOUT)? {
            let seconds = u64::try_from(value).map_err(|_| {
                DruidError::InvalidArgument(format!(
                    "{} must not be negative: {value}",
                    Self::PROP_VALIDATION_QUERY_TIMEOUT
                ))
            })?;
            builder = builder.validation_query_timeout(Duration::from_secs(seconds));
        }
        if let Some(value) = optional_i64(properties, Self::PROP_QUERY_TIMEOUT)? {
            builder = builder.query_timeout(i32_property(Self::PROP_QUERY_TIMEOUT, value)?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_TRANSACTION_QUERY_TIMEOUT)? {
            builder = builder.transaction_query_timeout(i32_property(
                Self::PROP_TRANSACTION_QUERY_TIMEOUT,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_LOGIN_TIMEOUT)? {
            builder = builder.login_timeout(i32_property(Self::PROP_LOGIN_TIMEOUT, value)?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_STAT_SQL_MAX_SIZE)? {
            builder = builder.max_sql_size(i32_property(Self::PROP_STAT_SQL_MAX_SIZE, value)?);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_REMOVE_ABANDONED)? {
            builder = builder.remove_abandoned(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_REMOVE_ABANDONED_TIMEOUT)? {
            let seconds = u64::try_from(value).map_err(|_| {
                DruidError::InvalidArgument(format!(
                    "{} must not be negative: {value}",
                    Self::PROP_REMOVE_ABANDONED_TIMEOUT
                ))
            })?;
            builder = builder.remove_abandoned_timeout(Duration::from_secs(seconds));
        }
        if let Some(value) = optional_bool(properties, Self::PROP_LOG_ABANDONED)? {
            builder = builder.log_abandoned(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_KEEP_ALIVE)? {
            builder = builder.keep_alive(value);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_KEEP_ALIVE_BETWEEN_TIME_MILLIS)? {
            builder = builder.keep_alive_between_time(non_negative_duration(
                Self::PROP_KEEP_ALIVE_BETWEEN_TIME_MILLIS,
                value,
            )?);
        }
        if let Some(value) = optional_i64(properties, Self::PROP_PHY_MAX_USE_COUNT)? {
            builder = builder.max_use_count(if value < 0 {
                0
            } else {
                usize::try_from(value).map_err(|_| {
                    DruidError::InvalidArgument(format!(
                        "{} is out of range: {value}",
                        Self::PROP_PHY_MAX_USE_COUNT
                    ))
                })?
            });
        }
        if let Some(value) = optional_bool(properties, Self::PROP_POOL_PREPARED_STATEMENTS)? {
            builder = builder.pool_prepared_statements(value);
        }
        if let Some(value) = optional_usize(properties, Self::PROP_MAX_OPEN_PREPARED_STATEMENTS)? {
            builder = builder.max_pool_prepared_statements_per_connection(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_SHARE_PREPARED_STATEMENTS)? {
            builder = builder.share_prepared_statements(value);
        }
        if let Some(value) = optional_bool(properties, Self::PROP_USE_ORACLE_IMPLICIT_CACHE)? {
            builder = builder.use_oracle_implicit_cache(value);
        }
        if let Some(value) = optional_bool(properties, "druid.clearFiltersEnable")? {
            builder = builder.clear_filters_enable(value);
        }
        if let Some(value) = optional_bool(properties, "druid.loadSpifilterSkip")? {
            builder = builder.load_spi_filter_skip(value);
        }
        builder.set_filters(properties.get(Self::PROP_FILTERS).map(String::as_str))?;
        let filter_properties = properties
            .get("connectionProperties")
            .map(|source| parse_connection_properties(source))
            .unwrap_or_default();
        let system_properties = stat_filter_system_properties();
        builder.configure_filters(&filter_properties, &system_properties)?;

        let data_source = builder.build_data_source().await?;
        if let Some(value) = optional_bool(properties, Self::PROP_RESET_STAT_ENABLE)? {
            data_source.set_reset_stat_enable(value);
        }
        if optional_bool(properties, Self::PROP_INIT)?.unwrap_or(false) {
            data_source.init().await?;
        }
        Ok(data_source)
    }

    pub const PROP_DEFAULT_AUTO_COMMIT: &'static str = "defaultAutoCommit";
    pub const PROP_DEFAULT_READ_ONLY: &'static str = "defaultReadOnly";
    pub const PROP_DEFAULT_TRANSACTION_ISOLATION: &'static str = "defaultTransactionIsolation";
    pub const PROP_DEFAULT_CATALOG: &'static str = "defaultCatalog";
    pub const PROP_INIT_VARIANTS: &'static str = "druid.initVariants";
    pub const PROP_INIT_GLOBAL_VARIANTS: &'static str = "druid.initGlobalVariants";
    pub const PROP_INIT_CONNECTION_SQLS: &'static str = "druid.initConnectionSqls";
    pub const PROP_DRIVER_CLASS_NAME: &'static str = "driverClassName";
    pub const PROP_DB_TYPE: &'static str = "dbType";
    pub const PROP_MAX_ACTIVE: &'static str = "maxActive";
    pub const PROP_MAX_IDLE: &'static str = "maxIdle";
    pub const PROP_MIN_IDLE: &'static str = "minIdle";
    pub const PROP_INITIAL_SIZE: &'static str = "initialSize";
    pub const PROP_ASYNC_INIT: &'static str = "druid.asyncInit";
    pub const PROP_INIT_EXCEPTION_THROW: &'static str = "druid.initExceptionThrow";
    pub const PROP_RESET_STAT_ENABLE: &'static str = "druid.resetStatEnable";
    pub const PROP_MAX_WAIT: &'static str = "maxWait";
    pub const PROP_NOT_FULL_TIMEOUT_RETRY_COUNT: &'static str = "druid.notFullTimeoutRetryCount";
    pub const PROP_MAX_WAIT_THREAD_COUNT: &'static str = "druid.maxWaitThreadCount";
    pub const PROP_CONNECTION_ERROR_RETRY_ATTEMPTS: &'static str =
        "druid.connectionErrorRetryAttempts";
    pub const PROP_BREAK_AFTER_ACQUIRE_FAILURE: &'static str = "druid.breakAfterAcquireFailure";
    pub const PROP_TIME_BETWEEN_CONNECT_ERROR_MILLIS: &'static str =
        "druid.timeBetweenConnectErrorMillis";
    pub const PROP_FAIL_FAST: &'static str = "druid.failFast";
    pub const PROP_ON_FATAL_ERROR_MAX_ACTIVE: &'static str = "druid.onFatalErrorMaxActive";
    pub const PROP_TEST_ON_BORROW: &'static str = "testOnBorrow";
    pub const PROP_TEST_ON_RETURN: &'static str = "testOnReturn";
    pub const PROP_TEST_WHILE_IDLE: &'static str = "testWhileIdle";
    pub const PROP_TIME_BETWEEN_EVICTION_RUNS_MILLIS: &'static str =
        "timeBetweenEvictionRunsMillis";
    pub const PROP_MIN_EVICTABLE_IDLE_TIME_MILLIS: &'static str = "minEvictableIdleTimeMillis";
    pub const PROP_MAX_EVICTABLE_IDLE_TIME_MILLIS: &'static str = "maxEvictableIdleTimeMillis";
    pub const PROP_PHY_TIMEOUT_MILLIS: &'static str = "phyTimeoutMillis";
    pub const PROP_USERNAME: &'static str = "username";
    pub const PROP_PASSWORD: &'static str = "password";
    pub const PROP_URL: &'static str = "url";
    pub const PROP_VALIDATION_QUERY: &'static str = "validationQuery";
    pub const PROP_VALIDATION_QUERY_TIMEOUT: &'static str = "validationQueryTimeout";
    pub const PROP_QUERY_TIMEOUT: &'static str = "queryTimeout";
    pub const PROP_TRANSACTION_QUERY_TIMEOUT: &'static str = "transactionQueryTimeout";
    pub const PROP_LOGIN_TIMEOUT: &'static str = "loginTimeout";
    pub const PROP_STAT_SQL_MAX_SIZE: &'static str = "druid.stat.sql.MaxSize";
    pub const PROP_REMOVE_ABANDONED: &'static str = "removeAbandoned";
    pub const PROP_REMOVE_ABANDONED_TIMEOUT: &'static str = "removeAbandonedTimeout";
    pub const PROP_LOG_ABANDONED: &'static str = "logAbandoned";
    pub const PROP_KEEP_ALIVE: &'static str = "keepAlive";
    pub const PROP_KEEP_ALIVE_BETWEEN_TIME_MILLIS: &'static str = "keepAliveBetweenTimeMillis";
    pub const PROP_PHY_MAX_USE_COUNT: &'static str = "phyMaxUseCount";
    pub const PROP_POOL_PREPARED_STATEMENTS: &'static str = "poolPreparedStatements";
    pub const PROP_MAX_OPEN_PREPARED_STATEMENTS: &'static str = "maxOpenPreparedStatements";
    pub const PROP_SHARE_PREPARED_STATEMENTS: &'static str = "sharePreparedStatements";
    pub const PROP_USE_ORACLE_IMPLICIT_CACHE: &'static str = "useOracleImplicitCache";
    pub const PROP_FILTERS: &'static str = "filters";
    pub const PROP_INIT: &'static str = "init";
    pub const PROP_NAME: &'static str = "name";
}

fn i32_property(name: &str, value: i64) -> Result<i32, DruidError> {
    i32::try_from(value).map_err(|_| {
        DruidError::InvalidArgument(format!("{name} is outside Java int range: {value}"))
    })
}

fn wall_config_from_properties(
    properties: &HashMap<String, String>,
) -> Result<Option<WallConfig>, DruidError> {
    const BOOLEAN_PROPERTIES: &[&str] = &[
        "druid.wall.selectAllow",
        // Java 1.2.x factory 的历史拼写必须继续接受。
        "druid.wall.selelctAllow",
        "druid.wall.selectAllColumnAllow",
        "druid.wall.selectIntoAllow",
        "druid.wall.insertAllow",
        "druid.wall.updateAllow",
        "druid.wall.deleteAllow",
        "druid.wall.dropTableAllow",
        "druid.wall.truncateAllow",
        "druid.wall.alterTableAllow",
        "druid.wall.createTableAllow",
        "druid.wall.commitAllow",
        "druid.wall.rollbackAllow",
        "druid.wall.startTransactionAllow",
        "druid.wall.setAllow",
        "druid.wall.updateWhereAlwayTrueCheck",
        "druid.wall.deleteWhereAlwayTrueCheck",
        "druid.wall.selectWhereAlwayTrueCheck",
        "druid.wall.selectHavingAlwayTrueCheck",
        "druid.wall.updateMustHaveWhere",
        "druid.wall.deleteMustHaveWhere",
        "druid.wall.multiStatementAllow",
        "druid.wall.commentAllow",
        "druid.wall.mustParameterized",
        "druid.wall.limitZeroAllow",
        "druid.wall.noneBaseStatementAllow",
    ];
    let configured = BOOLEAN_PROPERTIES
        .iter()
        .any(|name| properties.contains_key(*name))
        || properties.contains_key("druid.wall.tenantColumn")
        || properties.contains_key("druid.wall.tenantTablePattern");
    if !configured {
        return Ok(None);
    }

    let mut config = WallConfig::default();
    let read = |name: &'static str| optional_bool(properties, name);
    if let Some(value) = read("druid.wall.selectAllow")?.or(read("druid.wall.selelctAllow")?) {
        config.select_allow = value;
    }
    macro_rules! set_bool {
        ($property:literal, $field:ident) => {
            if let Some(value) = read($property)? {
                config.$field = value;
            }
        };
    }
    set_bool!("druid.wall.selectAllColumnAllow", select_all_column_allow);
    set_bool!("druid.wall.selectIntoAllow", select_into_allow);
    set_bool!("druid.wall.insertAllow", insert_allow);
    set_bool!("druid.wall.updateAllow", update_allow);
    set_bool!("druid.wall.deleteAllow", delete_allow);
    set_bool!("druid.wall.dropTableAllow", drop_table_allow);
    set_bool!("druid.wall.truncateAllow", truncate_allow);
    set_bool!("druid.wall.alterTableAllow", alter_table_allow);
    set_bool!("druid.wall.createTableAllow", create_table_allow);
    set_bool!("druid.wall.commitAllow", commit_allow);
    set_bool!("druid.wall.rollbackAllow", rollback_allow);
    set_bool!("druid.wall.startTransactionAllow", start_transaction_allow);
    set_bool!("druid.wall.setAllow", set_allow);
    set_bool!(
        "druid.wall.updateWhereAlwayTrueCheck",
        update_where_alway_true_check
    );
    set_bool!(
        "druid.wall.deleteWhereAlwayTrueCheck",
        delete_where_alway_true_check
    );
    set_bool!(
        "druid.wall.selectWhereAlwayTrueCheck",
        select_where_alway_true_check
    );
    set_bool!(
        "druid.wall.selectHavingAlwayTrueCheck",
        select_having_alway_true_check
    );
    set_bool!("druid.wall.updateMustHaveWhere", update_must_have_where);
    set_bool!("druid.wall.deleteMustHaveWhere", delete_must_have_where);
    set_bool!("druid.wall.multiStatementAllow", multi_statement_allow);
    set_bool!("druid.wall.commentAllow", comment_allow);
    set_bool!("druid.wall.mustParameterized", must_parameterized);
    set_bool!("druid.wall.limitZeroAllow", limit_zero_allow);
    set_bool!(
        "druid.wall.noneBaseStatementAllow",
        none_base_statement_allow
    );
    if let Some(value) = properties.get("druid.wall.tenantColumn") {
        config.tenant_column.clone_from(value);
    }
    if let Some(value) = properties.get("druid.wall.tenantTablePattern") {
        config.tenant_table_pattern.clone_from(value);
    }
    Ok(Some(config))
}

fn required<'a>(
    properties: &'a HashMap<String, String>,
    name: &'static str,
) -> Result<&'a str, DruidError> {
    properties
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| DruidError::InvalidArgument(format!("{name} is required")))
}

fn optional_bool(
    properties: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<bool>, DruidError> {
    properties
        .get(name)
        .map(|value| parse_bool(name, value))
        .transpose()
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, DruidError> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(DruidError::InvalidArgument(format!(
            "{name} must be true or false: {value}"
        ))),
    }
}

fn optional_i64(
    properties: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<i64>, DruidError> {
    properties
        .get(name)
        .map(|value| {
            value.parse::<i64>().map_err(|_| {
                DruidError::InvalidArgument(format!("{name} is not an integer: {value}"))
            })
        })
        .transpose()
}

fn optional_usize(
    properties: &HashMap<String, String>,
    name: &'static str,
) -> Result<Option<usize>, DruidError> {
    properties
        .get(name)
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                DruidError::InvalidArgument(format!(
                    "{name} is not a non-negative integer: {value}"
                ))
            })
        })
        .transpose()
}

fn non_negative_duration(name: &'static str, value: i64) -> Result<Duration, DruidError> {
    u64::try_from(value)
        .map(Duration::from_millis)
        .map_err(|_| DruidError::InvalidArgument(format!("{name} must not be negative: {value}")))
}

fn parse_connection_properties(source: &str) -> HashMap<String, String> {
    source
        .split(';')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry.find('=').filter(|index| *index > 0).map_or_else(
                || (entry.to_owned(), String::new()),
                |index| (entry[..index].to_owned(), entry[index + 1..].to_owned()),
            )
        })
        .collect()
}

fn physical_connection_properties(properties: &HashMap<String, String>) -> HashMap<String, String> {
    let mut connection_properties = properties
        .get("connectionProperties")
        .map(|source| parse_connection_properties(source))
        .unwrap_or_default();
    if let Some(user) = properties
        .get(DruidDataSourceFactory::PROP_USERNAME)
        .filter(|value| !value.is_empty())
    {
        connection_properties.insert("user".to_owned(), user.clone());
    }
    if let Some(password) = properties
        .get(DruidDataSourceFactory::PROP_PASSWORD)
        .filter(|value| !value.is_empty())
    {
        connection_properties.insert("password".to_owned(), password.clone());
    }
    connection_properties
}

fn stat_filter_system_properties() -> HashMap<String, String> {
    [
        "druid.stat.mergeSql",
        "druid.stat.slowSqlMillis",
        "druid.stat.logSlowSql",
        "druid.stat.slowSqlLogLevel",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
    .collect()
}

fn parse_transaction_isolation(value: &str) -> Result<Option<u8>, DruidError> {
    let level = match value.to_ascii_uppercase().as_str() {
        "NONE" => 0,
        "READ_UNCOMMITTED" => 1,
        "READ_COMMITTED" => 2,
        "REPEATABLE_READ" => 4,
        "SERIALIZABLE" => 8,
        _ => match value.parse::<i16>() {
            Ok(-1) => return Ok(None),
            Ok(value) => u8::try_from(value).map_err(|_| {
                DruidError::InvalidArgument(format!(
                    "defaultTransactionIsolation is out of range: {value}"
                ))
            })?,
            Err(_) => return Ok(None),
        },
    };
    Ok(Some(level))
}
