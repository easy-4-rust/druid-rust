use crate::sqlx::bb8::SqlxBb8Pool;
use crate::sqlx::deadpool::SqlxDeadpoolPool;
use crate::sqlx::SqlxConnectionFactory;
use crate::{ManagedWrapperPool, ProxoolConfigKey};
use druid_core::core::{DruidError, PhysicalConnectionFactory};
use druid_core::pool::DruidPoolBuilder;
use druid_core::sql::RdbcUtils;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Java wrapper 属性到 Rust 单池 provider 的统一工厂。
///
/// 规划迁移 DBCP/DBCP2 Factory、c3p0 与 Proxool `DataSource` 配置。provider
/// 可选 `native`、`bb8`、`deadpool`；native 使用 `SQLx` raw factory 进入
/// DruidPool，外部池则直接实现 Pool，绝不嵌套。
#[derive(Debug, Default, Clone, Copy)]
pub struct WrapperDataSourceFactory;

impl WrapperDataSourceFactory {
    /// 使用调用方提供的 raw connection factory 创建 native Druid wrapper。
    ///
    /// RBDC 及其他扩展通过此入口复用 DBCP/Proxool 属性语义；factory 每次只
    /// 创建一条物理连接，池化仍唯一属于 `DruidPool`。
    pub async fn create_with_factory(
        properties: &HashMap<String, String>,
        factory: Arc<dyn PhysicalConnectionFactory>,
        driver_name: impl Into<String>,
    ) -> Result<ManagedWrapperPool, DruidError> {
        let url = first(
            properties,
            &[
                ProxoolConfigKey::URL,
                ProxoolConfigKey::DRIVER_URL,
                ProxoolConfigKey::PROXOOL_DRIVER_URL,
            ],
        )
        .ok_or_else(|| DruidError::InvalidArgument("url is required".to_owned()))?;
        let name = first(
            properties,
            &[
                ProxoolConfigKey::NAME,
                ProxoolConfigKey::ALIAS,
                ProxoolConfigKey::PROXOOL_ALIAS,
            ],
        )
        .unwrap_or("druid-wrapper");
        let max_open = first_usize(
            properties,
            &[
                ProxoolConfigKey::MAX_ACTIVE,
                ProxoolConfigKey::MAX_TOTAL,
                ProxoolConfigKey::MAXIMUM_CONNECTION_COUNT,
                ProxoolConfigKey::PROXOOL_MAXIMUM_CONNECTION_COUNT,
            ],
        )?
        .unwrap_or(8);
        let acquire_timeout = Duration::from_millis(
            first_u64(
                properties,
                &[
                    ProxoolConfigKey::MAX_WAIT_MILLIS,
                    ProxoolConfigKey::MAX_WAIT,
                ],
            )?
            .unwrap_or(30_000),
        );
        if max_open == 0 {
            return Err(DruidError::InvalidArgument(
                "maximum connection count must be greater than zero".to_owned(),
            ));
        }
        let configured_driver_name = first(
            properties,
            &[
                ProxoolConfigKey::DRIVER_CLASS,
                ProxoolConfigKey::PROXOOL_DRIVER_CLASS,
            ],
        )
        .map_or_else(|| driver_name.into(), str::to_owned);
        let mut builder = DruidPoolBuilder::new()
            .name(name)
            .driver_name(&configured_driver_name)
            .factory(factory)
            .max_open(max_open)
            .acquire_timeout(acquire_timeout);
        if let Some(db_type) = RdbcUtils::infer_db_type(Some(url), Some(&configured_driver_name)) {
            builder = builder.db_type_name(db_type.as_str());
        }
        if let Some(value) = first_usize(
            properties,
            &[
                ProxoolConfigKey::MIN_IDLE,
                ProxoolConfigKey::MINIMUM_CONNECTION_COUNT,
                ProxoolConfigKey::PROXOOL_MINIMUM_CONNECTION_COUNT,
            ],
        )? {
            builder = builder.min_idle(value);
        }
        if let Some(value) = first_usize(properties, &[ProxoolConfigKey::INITIAL_SIZE])? {
            builder = builder.initial_size(value);
        }
        if let Some(value) = first_bool(
            properties,
            &[
                ProxoolConfigKey::TEST_ON_BORROW,
                ProxoolConfigKey::TEST_BEFORE_USE,
                ProxoolConfigKey::PROXOOL_TEST_BEFORE_USE,
            ],
        )? {
            builder = builder.test_on_borrow(value);
        }
        if let Some(value) = first_bool(
            properties,
            &[
                ProxoolConfigKey::TEST_ON_RETURN,
                ProxoolConfigKey::TEST_AFTER_USE,
                ProxoolConfigKey::PROXOOL_TEST_AFTER_USE,
            ],
        )? {
            builder = builder.test_on_return(value);
        }
        if let Some(value) = first_bool(properties, &[ProxoolConfigKey::TEST_WHILE_IDLE])? {
            builder = builder.test_while_idle(value);
        }
        if let Some(value) = first(
            properties,
            &[
                ProxoolConfigKey::VALIDATION_QUERY,
                ProxoolConfigKey::HOUSE_KEEPING_TEST_SQL,
                ProxoolConfigKey::PROXOOL_HOUSE_KEEPING_TEST_SQL,
            ],
        ) {
            builder = builder.validation_query(value);
        }
        if let Some(value) = first_u64(
            properties,
            &[
                ProxoolConfigKey::HOUSE_KEEPING_SLEEP_TIME,
                ProxoolConfigKey::PROXOOL_HOUSE_KEEPING_SLEEP_TIME,
            ],
        )? {
            builder = builder.time_between_eviction_runs(Duration::from_millis(value));
        }
        if let Some(value) = first_u64(
            properties,
            &[
                ProxoolConfigKey::MAXIMUM_ACTIVE_TIME,
                ProxoolConfigKey::PROXOOL_MAXIMUM_ACTIVE_TIME,
            ],
        )? {
            builder = builder
                .remove_abandoned(true)
                .remove_abandoned_timeout(Duration::from_millis(value));
        }
        Ok(ManagedWrapperPool::with_shutdown(
            "native",
            Arc::new(builder.build_data_source().await?),
        ))
    }

    /// 根据兼容属性创建 managed wrapper pool。
    pub async fn create(
        properties: &HashMap<String, String>,
    ) -> Result<ManagedWrapperPool, DruidError> {
        let provider = properties
            .get(ProxoolConfigKey::PROVIDER)
            .map_or("native", String::as_str)
            .to_ascii_lowercase();
        let url = first(
            properties,
            &[
                ProxoolConfigKey::URL,
                ProxoolConfigKey::DRIVER_URL,
                ProxoolConfigKey::PROXOOL_DRIVER_URL,
            ],
        )
        .ok_or_else(|| DruidError::InvalidArgument("url is required".to_owned()))?;
        let name = first(
            properties,
            &[
                ProxoolConfigKey::NAME,
                ProxoolConfigKey::ALIAS,
                ProxoolConfigKey::PROXOOL_ALIAS,
            ],
        )
        .unwrap_or("druid-wrapper");
        let max_open = first_usize(
            properties,
            &[
                ProxoolConfigKey::MAX_ACTIVE,
                ProxoolConfigKey::MAX_TOTAL,
                ProxoolConfigKey::MAXIMUM_CONNECTION_COUNT,
                ProxoolConfigKey::PROXOOL_MAXIMUM_CONNECTION_COUNT,
            ],
        )?
        .unwrap_or(8);
        if max_open == 0 {
            return Err(DruidError::InvalidArgument(
                "maximum connection count must be greater than zero".to_owned(),
            ));
        }
        let acquire_timeout = Duration::from_millis(
            first_u64(
                properties,
                &[
                    ProxoolConfigKey::MAX_WAIT_MILLIS,
                    ProxoolConfigKey::MAX_WAIT,
                ],
            )?
            .unwrap_or(30_000),
        );

        let native_url = RdbcUtils::to_rust_url(url).ok_or_else(|| {
            DruidError::InvalidArgument(format!(
                "SQLx provider does not support JDBC URL `{url}`; inject a matching PhysicalConnectionFactory"
            ))
        })?;
        let sqlx_url = apply_credentials(native_url.as_ref(), properties)?;
        let managed = match provider.as_str() {
            "native" | "druid" | "sqlx" => {
                let mut builder = DruidPoolBuilder::new()
                    .name(name)
                    .driver_name("sqlx")
                    .factory(Arc::new(SqlxConnectionFactory::new(&sqlx_url)))
                    .max_open(max_open)
                    .acquire_timeout(acquire_timeout);
                if let Some(db_type) = RdbcUtils::infer_db_type(Some(&sqlx_url), Some("sqlx")) {
                    builder = builder.db_type_name(db_type.as_str());
                }
                if let Some(value) = first_usize(
                    properties,
                    &[
                        ProxoolConfigKey::MIN_IDLE,
                        ProxoolConfigKey::MINIMUM_CONNECTION_COUNT,
                        ProxoolConfigKey::PROXOOL_MINIMUM_CONNECTION_COUNT,
                    ],
                )? {
                    builder = builder.min_idle(value);
                }
                if let Some(value) = first_usize(properties, &[ProxoolConfigKey::INITIAL_SIZE])? {
                    builder = builder.initial_size(value);
                }
                if let Some(value) = first_bool(
                    properties,
                    &[
                        ProxoolConfigKey::TEST_ON_BORROW,
                        ProxoolConfigKey::TEST_BEFORE_USE,
                        ProxoolConfigKey::PROXOOL_TEST_BEFORE_USE,
                    ],
                )? {
                    builder = builder.test_on_borrow(value);
                }
                if let Some(value) = first_bool(
                    properties,
                    &[
                        ProxoolConfigKey::TEST_ON_RETURN,
                        ProxoolConfigKey::TEST_AFTER_USE,
                        ProxoolConfigKey::PROXOOL_TEST_AFTER_USE,
                    ],
                )? {
                    builder = builder.test_on_return(value);
                }
                if let Some(value) = first_bool(properties, &[ProxoolConfigKey::TEST_WHILE_IDLE])? {
                    builder = builder.test_while_idle(value);
                }
                if let Some(value) = first(
                    properties,
                    &[
                        ProxoolConfigKey::VALIDATION_QUERY,
                        ProxoolConfigKey::HOUSE_KEEPING_TEST_SQL,
                        ProxoolConfigKey::PROXOOL_HOUSE_KEEPING_TEST_SQL,
                    ],
                ) {
                    builder = builder.validation_query(value);
                }
                if let Some(value) = first_u64(
                    properties,
                    &[
                        ProxoolConfigKey::HOUSE_KEEPING_SLEEP_TIME,
                        ProxoolConfigKey::PROXOOL_HOUSE_KEEPING_SLEEP_TIME,
                    ],
                )? {
                    builder = builder.time_between_eviction_runs(Duration::from_millis(value));
                }
                if let Some(value) = first_u64(
                    properties,
                    &[
                        ProxoolConfigKey::MAXIMUM_ACTIVE_TIME,
                        ProxoolConfigKey::PROXOOL_MAXIMUM_ACTIVE_TIME,
                    ],
                )? {
                    builder = builder
                        .remove_abandoned(true)
                        .remove_abandoned_timeout(Duration::from_millis(value));
                }
                ManagedWrapperPool::with_shutdown(
                    provider.clone(),
                    Arc::new(builder.build_data_source().await?),
                )
            }
            "bb8" => ManagedWrapperPool::new(
                provider.clone(),
                Arc::new(
                    SqlxBb8Pool::connect(name, &sqlx_url, max_open, acquire_timeout, None).await?,
                ),
            ),
            "deadpool" => ManagedWrapperPool::with_shutdown(
                provider.clone(),
                Arc::new(SqlxDeadpoolPool::connect(
                    name,
                    &sqlx_url,
                    max_open,
                    acquire_timeout,
                    None,
                )?),
            ),
            value => {
                return Err(DruidError::InvalidArgument(format!(
                    "unsupported wrapper provider: {value}"
                )));
            }
        };
        Ok(managed)
    }
}

fn apply_credentials(
    raw_url: &str,
    properties: &HashMap<String, String>,
) -> Result<String, DruidError> {
    let username = first(
        properties,
        &[ProxoolConfigKey::USERNAME, ProxoolConfigKey::USER],
    );
    let password = first(properties, &[ProxoolConfigKey::PASSWORD]);
    if username.is_none() && password.is_none() {
        return Ok(raw_url.to_owned());
    }
    let mut url = url::Url::parse(raw_url).map_err(|error| {
        DruidError::InvalidArgument(format!("invalid database URL for credentials: {error}"))
    })?;
    if url.scheme() == "sqlite" {
        return Err(DruidError::InvalidArgument(
            "SQLite does not accept username/password properties".to_owned(),
        ));
    }
    if let Some(username) = username {
        url.set_username(username).map_err(|()| {
            DruidError::InvalidArgument("database URL cannot carry username".to_owned())
        })?;
    }
    if let Some(password) = password {
        url.set_password(Some(password)).map_err(|()| {
            DruidError::InvalidArgument("database URL cannot carry password".to_owned())
        })?;
    }
    Ok(url.into())
}

fn first<'a>(properties: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| properties.get(*key))
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn first_usize(
    properties: &HashMap<String, String>,
    keys: &[&str],
) -> Result<Option<usize>, DruidError> {
    first(properties, keys)
        .map(|value| {
            value.parse().map_err(|_| {
                DruidError::InvalidArgument(format!("{} must be a non-negative integer", keys[0]))
            })
        })
        .transpose()
}

fn first_u64(
    properties: &HashMap<String, String>,
    keys: &[&str],
) -> Result<Option<u64>, DruidError> {
    first(properties, keys)
        .map(|value| {
            value.parse().map_err(|_| {
                DruidError::InvalidArgument(format!("{} must be a non-negative integer", keys[0]))
            })
        })
        .transpose()
}

fn first_bool(
    properties: &HashMap<String, String>,
    keys: &[&str],
) -> Result<Option<bool>, DruidError> {
    first(properties, keys)
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(DruidError::InvalidArgument(format!(
                "{} must be true or false",
                keys[0]
            ))),
        })
        .transpose()
}
