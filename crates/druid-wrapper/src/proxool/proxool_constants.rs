/// Proxool 属性常量。
///
/// 对应 Java: `org.logicalcobwebs.proxool.ProxoolConstants`。保留数据源实际
/// 读取和 Druid 映射所需的原始键及 `proxool.` 前缀键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxoolConstants;

impl ProxoolConstants {
    pub const PROXOOL: &'static str = "proxool";
    pub const PROPERTY_PREFIX: &'static str = "proxool.";
    pub const ALIAS: &'static str = "alias";
    pub const ALIAS_PROPERTY: &'static str = "proxool.alias";
    pub const DRIVER_CLASS: &'static str = "driver-class";
    pub const DRIVER_CLASS_PROPERTY: &'static str = "proxool.driver-class";
    pub const DRIVER_URL: &'static str = "driver-url";
    pub const DRIVER_URL_PROPERTY: &'static str = "proxool.driver-url";
    pub const USER_PROPERTY: &'static str = "user";
    pub const PASSWORD_PROPERTY: &'static str = "password";
    pub const HOUSE_KEEPING_SLEEP_TIME: &'static str = "house-keeping-sleep-time";
    pub const HOUSE_KEEPING_SLEEP_TIME_PROPERTY: &'static str = "proxool.house-keeping-sleep-time";
    pub const HOUSE_KEEPING_TEST_SQL: &'static str = "house-keeping-test-sql";
    pub const HOUSE_KEEPING_TEST_SQL_PROPERTY: &'static str = "proxool.house-keeping-test-sql";
    pub const TEST_BEFORE_USE: &'static str = "test-before-use";
    pub const TEST_BEFORE_USE_PROPERTY: &'static str = "proxool.test-before-use";
    pub const TEST_AFTER_USE: &'static str = "test-after-use";
    pub const TEST_AFTER_USE_PROPERTY: &'static str = "proxool.test-after-use";
    pub const MAXIMUM_CONNECTION_COUNT: &'static str = "maximum-connection-count";
    pub const MAXIMUM_CONNECTION_COUNT_PROPERTY: &'static str = "proxool.maximum-connection-count";
    pub const MINIMUM_CONNECTION_COUNT: &'static str = "minimum-connection-count";
    pub const MINIMUM_CONNECTION_COUNT_PROPERTY: &'static str = "proxool.minimum-connection-count";
    pub const MAXIMUM_CONNECTION_LIFETIME: &'static str = "maximum-connection-lifetime";
    pub const MAXIMUM_CONNECTION_LIFETIME_PROPERTY: &'static str =
        "proxool.maximum-connection-lifetime";
    pub const PROTOTYPE_COUNT: &'static str = "prototype-count";
    pub const PROTOTYPE_COUNT_PROPERTY: &'static str = "proxool.prototype-count";
    pub const SIMULTANEOUS_BUILD_THROTTLE: &'static str = "simultaneous-build-throttle";
    pub const SIMULTANEOUS_BUILD_THROTTLE_PROPERTY: &'static str =
        "proxool.simultaneous-build-throttle";
    pub const MAXIMUM_ACTIVE_TIME: &'static str = "maximum-active-time";
    pub const MAXIMUM_ACTIVE_TIME_PROPERTY: &'static str = "proxool.maximum-active-time";
}
