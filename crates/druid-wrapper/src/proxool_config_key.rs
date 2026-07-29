/// Proxool/DBCP/c3p0 常用属性名。
///
/// 对应 Java：`org.logicalcobwebs.proxool.ProxoolConstants`，并集中记录三类
/// wrapper 在 Rust factory 中接受的兼容键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxoolConfigKey;

impl ProxoolConfigKey {
    pub const URL: &'static str = "url";
    pub const DRIVER_URL: &'static str = "driver-url";
    pub const PROXOOL_DRIVER_URL: &'static str = "proxool.driver-url";
    pub const DRIVER_CLASS: &'static str = "driver-class";
    pub const PROXOOL_DRIVER_CLASS: &'static str = "proxool.driver-class";
    pub const NAME: &'static str = "name";
    pub const ALIAS: &'static str = "alias";
    pub const PROXOOL_ALIAS: &'static str = "proxool.alias";
    pub const PROVIDER: &'static str = "druid.wrapper.provider";
    pub const MAX_ACTIVE: &'static str = "maxActive";
    pub const MAX_TOTAL: &'static str = "maxTotal";
    pub const MAXIMUM_CONNECTION_COUNT: &'static str = "maximum-connection-count";
    pub const PROXOOL_MAXIMUM_CONNECTION_COUNT: &'static str = "proxool.maximum-connection-count";
    pub const MIN_IDLE: &'static str = "minIdle";
    pub const MINIMUM_CONNECTION_COUNT: &'static str = "minimum-connection-count";
    pub const PROXOOL_MINIMUM_CONNECTION_COUNT: &'static str = "proxool.minimum-connection-count";
    pub const INITIAL_SIZE: &'static str = "initialSize";
    pub const MAX_WAIT: &'static str = "maxWait";
    pub const MAX_WAIT_MILLIS: &'static str = "maxWaitMillis";
    pub const TEST_ON_BORROW: &'static str = "testOnBorrow";
    pub const TEST_ON_RETURN: &'static str = "testOnReturn";
    pub const TEST_WHILE_IDLE: &'static str = "testWhileIdle";
    pub const VALIDATION_QUERY: &'static str = "validationQuery";
    pub const HOUSE_KEEPING_TEST_SQL: &'static str = "house-keeping-test-sql";
    pub const PROXOOL_HOUSE_KEEPING_TEST_SQL: &'static str = "proxool.house-keeping-test-sql";
    pub const HOUSE_KEEPING_SLEEP_TIME: &'static str = "house-keeping-sleep-time";
    pub const PROXOOL_HOUSE_KEEPING_SLEEP_TIME: &'static str = "proxool.house-keeping-sleep-time";
    pub const MAXIMUM_ACTIVE_TIME: &'static str = "maximum-active-time";
    pub const PROXOOL_MAXIMUM_ACTIVE_TIME: &'static str = "proxool.maximum-active-time";
    pub const TEST_BEFORE_USE: &'static str = "test-before-use";
    pub const PROXOOL_TEST_BEFORE_USE: &'static str = "proxool.test-before-use";
    pub const TEST_AFTER_USE: &'static str = "test-after-use";
    pub const PROXOOL_TEST_AFTER_USE: &'static str = "proxool.test-after-use";
    pub const USER: &'static str = "user";
    pub const USERNAME: &'static str = "username";
    pub const PASSWORD: &'static str = "password";
}
