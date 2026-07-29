//! JDBC 日期时间重载的 Calendar 参数。
//!
//! 对应 Java 平台对象：`java.util.Calendar`。JDBC 日期时间 setter/getter
//! 使用 Calendar 的时区解释数据库值；Rust 侧保留时区标识以及“是否调用了
//! Calendar 重载”，由具体驱动完成时区规则解析。

use super::DruidError;

/// JDBC 日期时间转换使用的 Calendar 时区。
///
/// 对应 Java：`java.util.Calendar`。这里保留 IANA 时区标识或驱动接受的
/// 等价标识，不把它压缩为当前瞬时 UTC offset，以免丢失夏令时规则。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JdbcCalendar {
    time_zone_id: String,
}

impl JdbcCalendar {
    /// 创建 Calendar 时区描述。
    ///
    /// # 参数
    /// - `time_zone_id`：例如 `Asia/Shanghai`、`UTC`。
    ///
    /// # 返回
    /// 非空时区标识；空值返回结构化驱动错误。
    pub fn new(time_zone_id: impl Into<String>) -> Result<Self, DruidError> {
        let time_zone_id = time_zone_id.into();
        if time_zone_id.trim().is_empty() {
            return Err(DruidError::DriverError(
                "JDBC Calendar time zone id must not be empty".to_string(),
            ));
        }
        Ok(Self { time_zone_id })
    }

    /// 返回未改写的时区标识。
    pub fn time_zone_id(&self) -> &str {
        &self.time_zone_id
    }
}

/// Calendar 参数的重载状态。
///
/// `Unspecified` 表示调用无 Calendar 的重载；`Specified(None)` 表示调用
/// Calendar 重载但 Java 参数为 null。二者必须区分，避免重载身份丢失。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum JdbcCalendarArgument {
    /// 未调用 Calendar 重载。
    #[default]
    Unspecified,
    /// 调用了 Calendar 重载；内部值允许对应 Java null。
    Specified(Option<JdbcCalendar>),
}

impl JdbcCalendarArgument {
    /// 创建无 Calendar 的重载标记。
    pub fn unspecified() -> Self {
        Self::Unspecified
    }

    /// 创建显式 Calendar 重载标记。
    ///
    /// # 参数
    /// - `calendar`：Calendar 值；`None` 对应 Java null。
    pub fn specified(calendar: Option<JdbcCalendar>) -> Self {
        Self::Specified(calendar)
    }
}
