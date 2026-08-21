//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameter`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/proxy/rdbc/RdbcParameter.java`。

use super::{RdbcCalendar, RdbcInputStream, RdbcObject, RdbcReader};

/// Java `RdbcParameter` 自定义参数类型编号。
pub const BINARY_INPUT_STREAM: i32 = 10_001;
/// ASCII 输入流。
pub const ASCII_INPUT_STREAM: i32 = 10_002;
/// 字符输入流。
pub const CHARACTER_INPUT_STREAM: i32 = 10_003;
/// national-character 输入流。
pub const NCHARACTER_INPUT_STREAM: i32 = 10_004;
/// URL 参数。
pub const URL: i32 = 10_005;

/// Java 内部 `RdbcParameter.TYPE` 常量容器。
///
/// 该内部类型没有实例状态，保留在主对象文件中。
pub struct RdbcParameterType;

impl RdbcParameterType {
    /// 二进制输入流。
    pub const BINARY_INPUT_STREAM: i32 = BINARY_INPUT_STREAM;
    /// ASCII 输入流。
    pub const ASCII_INPUT_STREAM: i32 = ASCII_INPUT_STREAM;
    /// 字符输入流。
    pub const CHARACTER_INPUT_STREAM: i32 = CHARACTER_INPUT_STREAM;
    /// national-character 输入流。
    pub const NCHARACTER_INPUT_STREAM: i32 = NCHARACTER_INPUT_STREAM;
    /// URL。
    pub const URL: i32 = URL;
    /// 已废弃 Unicode 输入流。
    pub const UNICODE_STREAM: i32 = 10_006;
    /// byte[]。
    pub const BYTES: i32 = 10_007;
}

/// `RdbcParameter#getValue()` 的 Rust 无损平台值。
///
/// Java 返回 `Object`，既可能是标量/LOB，也可能是有状态 `InputStream` 或
/// Reader；Rust 必须显式区分这些资源句柄，不能提前读取。
#[derive(Debug, Clone, PartialEq)]
pub enum RdbcParameterValue {
    /// 标量、LOB 或 vendor RDBC 对象。
    Object(RdbcObject),
    /// 有状态字节输入流。
    InputStream(RdbcInputStream),
    /// 有状态 UTF-16 字符 Reader。
    Reader(RdbcReader),
}

/// `PreparedStatement` 参数的可观察代理合同。
pub trait RdbcParameter {
    /// 返回参数值；`None` 对应 Java null。
    fn value(&self) -> Option<RdbcParameterValue>;

    /// 返回声明长度。
    ///
    /// Java 特化对象返回 0，通用对象未声明长度时返回 -1。
    fn length(&self) -> i64;

    /// 返回 Calendar；`None` 对应 Java null 或未使用 Calendar 重载。
    fn calendar(&self) -> Option<RdbcCalendar>;

    /// 返回 `java.sql.Types` 或 Druid 自定义类型编号。
    fn sql_type(&self) -> i32;
}
