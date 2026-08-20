use crate::{ManagedWrapperPool, ProxoolConfigKey, WrapperDataSourceFactory};
use druid_core::core::DruidError;
use std::collections::HashMap;

/// Proxool 兼容数据源配置对象。
///
/// 对应 Java: `org.logicalcobwebs.proxool.ProxoolDataSource`。Java 对象把
/// Proxool setter 收集为属性后委托 Druid；Rust 同样先保存配置，再显式异步
/// `build()`，最终只创建一个 provider pool。
#[derive(Default, Clone)]
pub struct ProxoolDataSource {
    properties: HashMap<String, String>,
}

impl ProxoolDataSource {
    /// 创建空配置。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用 alias 创建配置。
    #[must_use]
    pub fn with_alias(alias: impl Into<String>) -> Self {
        let mut value = Self::new();
        value.set_alias(alias);
        value
    }

    /// 返回 alias。
    #[must_use]
    pub fn alias(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::ALIAS)
            .map(String::as_str)
    }

    /// 设置 alias。
    pub fn set_alias(&mut self, alias: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::ALIAS.to_owned(), alias.into());
    }

    /// 返回 driver URL。
    #[must_use]
    pub fn driver_url(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::DRIVER_URL)
            .map(String::as_str)
    }

    /// 设置 driver URL。
    pub fn set_driver_url(&mut self, url: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::DRIVER_URL.to_owned(), url.into());
    }

    /// 返回驱动类名。
    #[must_use]
    pub fn driver(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::DRIVER_CLASS)
            .map(String::as_str)
    }

    /// 设置驱动类名。
    pub fn set_driver(&mut self, driver: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::DRIVER_CLASS.to_owned(), driver.into());
    }

    /// Java wrapper 固定返回 `Long.MAX_VALUE`。
    #[must_use]
    pub const fn maximum_connection_lifetime(&self) -> i64 {
        i64::MAX
    }

    /// Java 方法已废弃且为空实现。
    pub const fn set_maximum_connection_lifetime(&mut self, _value: i32) {}

    /// Java wrapper 固定返回 0。
    #[must_use]
    pub const fn prototype_count(&self) -> i32 {
        0
    }

    /// Java 方法已废弃且为空实现。
    pub const fn set_prototype_count(&mut self, _value: i32) {}

    /// 设置最大连接数。
    pub fn set_maximum_connection_count(&mut self, count: usize) {
        self.properties.insert(
            ProxoolConfigKey::MAXIMUM_CONNECTION_COUNT.to_owned(),
            count.to_string(),
        );
    }

    /// 返回最大连接数，未设置时采用 Druid 默认 8。
    #[must_use]
    pub fn maximum_connection_count(&self) -> usize {
        property_usize(
            &self.properties,
            ProxoolConfigKey::MAXIMUM_CONNECTION_COUNT,
            8,
        )
    }

    /// 设置最小连接数。
    pub fn set_minimum_connection_count(&mut self, count: usize) {
        self.properties.insert(
            ProxoolConfigKey::MINIMUM_CONNECTION_COUNT.to_owned(),
            count.to_string(),
        );
    }

    /// 返回最小空闲连接数。
    #[must_use]
    pub fn minimum_connection_count(&self) -> usize {
        property_usize(
            &self.properties,
            ProxoolConfigKey::MINIMUM_CONNECTION_COUNT,
            0,
        )
    }

    /// 返回 housekeeping 周期毫秒。
    #[must_use]
    pub fn house_keeping_sleep_time(&self) -> u64 {
        property_u64(
            &self.properties,
            ProxoolConfigKey::HOUSE_KEEPING_SLEEP_TIME,
            60_000,
        )
    }

    /// 设置 housekeeping 周期毫秒。
    pub fn set_house_keeping_sleep_time(&mut self, millis: u64) {
        self.properties.insert(
            ProxoolConfigKey::HOUSE_KEEPING_SLEEP_TIME.to_owned(),
            millis.to_string(),
        );
    }

    /// Java wrapper 固定返回 0。
    #[must_use]
    pub const fn simultaneous_build_throttle(&self) -> i32 {
        0
    }

    /// Java 空实现。
    pub const fn set_simultaneous_build_throttle(&mut self, _value: i32) {}

    /// Java wrapper 固定返回 0。
    #[must_use]
    pub const fn recently_started_threshold(&self) -> i64 {
        0
    }

    /// Java 空实现。
    pub const fn set_recently_started_threshold(&mut self, _value: i32) {}

    /// Java wrapper 固定返回 0。
    #[must_use]
    pub const fn overload_without_refusal_lifetime(&self) -> i64 {
        0
    }

    /// Java 空实现。
    pub const fn set_overload_without_refusal_lifetime(&mut self, _value: i32) {}

    /// 返回 remove-abandoned 超时毫秒。
    #[must_use]
    pub fn maximum_active_time(&self) -> u64 {
        property_u64(
            &self.properties,
            ProxoolConfigKey::MAXIMUM_ACTIVE_TIME,
            300_000,
        )
    }

    /// 设置 remove-abandoned 超时毫秒。
    pub fn set_maximum_active_time(&mut self, millis: u64) {
        self.properties.insert(
            ProxoolConfigKey::MAXIMUM_ACTIVE_TIME.to_owned(),
            millis.to_string(),
        );
    }

    /// Java wrapper 固定关闭 verbose。
    #[must_use]
    pub const fn is_verbose(&self) -> bool {
        false
    }

    /// Java 已废弃空实现。
    pub const fn set_verbose(&mut self, _value: bool) {}

    /// Java wrapper 固定关闭 trace。
    #[must_use]
    pub const fn is_trace(&self) -> bool {
        false
    }

    /// Java 已废弃空实现。
    pub const fn set_trace(&mut self, _value: bool) {}

    /// Java wrapper 固定返回空统计配置。
    #[must_use]
    pub const fn statistics(&self) -> &'static str {
        ""
    }

    /// Java 空实现。
    pub fn set_statistics(&mut self, _value: impl Into<String>) {}

    /// Java wrapper 固定返回空日志级别。
    #[must_use]
    pub const fn statistics_log_level(&self) -> &'static str {
        ""
    }

    /// Java 空实现。
    pub fn set_statistics_log_level(&mut self, _value: impl Into<String>) {}

    /// Java wrapper 不提供 fatal SQL exception 列表。
    #[must_use]
    pub const fn fatal_sql_exceptions_as_string(&self) -> Option<&str> {
        None
    }

    /// Java 已废弃空实现。
    pub fn set_fatal_sql_exceptions_as_string(&mut self, _value: impl Into<String>) {}

    /// Java wrapper 不提供异常包装类。
    #[must_use]
    pub const fn fatal_sql_exception_wrapper_class(&self) -> Option<&str> {
        None
    }

    /// Java 空实现。
    pub fn set_fatal_sql_exception_wrapper_class(&mut self, _value: impl Into<String>) {}

    /// 设置借出前校验。
    pub fn set_test_before_use(&mut self, enabled: bool) {
        self.properties.insert(
            ProxoolConfigKey::TEST_ON_BORROW.to_owned(),
            enabled.to_string(),
        );
    }

    /// 设置归还后校验。
    pub fn set_test_after_use(&mut self, enabled: bool) {
        self.properties.insert(
            ProxoolConfigKey::TEST_ON_RETURN.to_owned(),
            enabled.to_string(),
        );
    }

    /// 设置 housekeeping 验证 SQL。
    pub fn set_house_keeping_test_sql(&mut self, sql: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::VALIDATION_QUERY.to_owned(), sql.into());
    }

    /// 返回 housekeeping SQL。
    #[must_use]
    pub fn house_keeping_test_sql(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::VALIDATION_QUERY)
            .map(String::as_str)
    }

    /// 返回用户名。
    #[must_use]
    pub fn user(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::USER)
            .map(String::as_str)
    }

    /// 设置用户名。
    pub fn set_user(&mut self, user: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::USER.to_owned(), user.into());
    }

    /// 返回密码。
    #[must_use]
    pub fn password(&self) -> Option<&str> {
        self.properties
            .get(ProxoolConfigKey::PASSWORD)
            .map(String::as_str)
    }

    /// 设置密码。
    pub fn set_password(&mut self, password: impl Into<String>) {
        self.properties
            .insert(ProxoolConfigKey::PASSWORD.to_owned(), password.into());
    }

    /// Java wrapper 固定启用 JMX 管理语义。
    #[must_use]
    pub const fn is_jmx(&self) -> bool {
        true
    }

    /// Java 空实现。
    pub const fn set_jmx(&mut self, _value: bool) {}

    /// Java wrapper 固定返回空 agent id。
    #[must_use]
    pub const fn jmx_agent_id(&self) -> &'static str {
        ""
    }

    /// Java 已废弃空实现。
    pub fn set_jmx_agent_id(&mut self, _value: impl Into<String>) {}

    /// 返回借出前校验开关。
    #[must_use]
    pub fn is_test_before_use(&self) -> bool {
        property_bool(&self.properties, ProxoolConfigKey::TEST_ON_BORROW, false)
    }

    /// 返回归还后校验开关。
    #[must_use]
    pub fn is_test_after_use(&self) -> bool {
        property_bool(&self.properties, ProxoolConfigKey::TEST_ON_RETURN, false)
    }

    /// 写入额外兼容属性。
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(name.into(), value.into());
    }

    /// 解析 Java `setDelegateProperties` 的逗号分隔 `name=value` 文本。
    pub fn set_delegate_properties(&mut self, properties: &str) -> Result<(), DruidError> {
        for pair in properties.split(',').filter(|value| !value.is_empty()) {
            let tokens = pair.split('=').collect::<Vec<_>>();
            match tokens.as_slice() {
                [name] => {
                    self.properties
                        .insert(name.trim().to_owned(), String::new());
                }
                [name, value] => {
                    self.properties
                        .insert(name.trim().to_owned(), value.trim().to_owned());
                }
                _ => {
                    return Err(DruidError::InvalidArgument(format!(
                        "Unexpected delegateProperties value: '{properties}'. Expected 'name=value'"
                    )));
                }
            }
        }
        Ok(())
    }

    /// 返回全部属性。
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// 创建唯一 managed provider pool。
    pub async fn build(&self) -> Result<ManagedWrapperPool, DruidError> {
        WrapperDataSourceFactory::create(&self.properties).await
    }
}

fn property_usize(properties: &HashMap<String, String>, name: &str, default_value: usize) -> usize {
    properties
        .get(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default_value)
}

fn property_u64(properties: &HashMap<String, String>, name: &str, default_value: u64) -> u64 {
    properties
        .get(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default_value)
}

fn property_bool(properties: &HashMap<String, String>, name: &str, default_value: bool) -> bool {
    properties
        .get(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default_value)
}
