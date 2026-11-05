//! `java.sql.Timestamp`、`java.sql.Date`、`java.sql.Time` 的薄扩展 trait。
//!
//! 对应 Java: `java.sql.Timestamp`、`java.sql.Date`、`java.sql.Time`。
//!
//! 这三个 trait 把 JDBC 三个日期时间类型的可观察语义直接暴露在 chrono 的
//! `NaiveDateTime`、`NaiveDate`、`NaiveTime` 上，不引入新的包装类型：
//!
//! - 文本形态严格对齐 JDK 8：`Timestamp` 为 `yyyy-mm-dd hh:mm:ss.fffffffff`，
//!   `Date` 为 `yyyy-mm-dd`，`Time` 为 `hh:mm:ss`。
//! - 纳秒字段在 `Timestamp#toString` 中按 JDK 8 规则补零后裁掉末尾的零，
//!   纳秒为零时输出单个 `0`。
//! - `Timestamp` 的 `after`/`before`/`compareTo` 先比较毫秒再比较纳秒。
//! - 与时区相关的入口（`toInstant`、`from`、`setTime`、`getTime`）按进程默认时区
//!   解释本地字段，与 `java.util.Date` 家族一致。
//!
//! 与 JDK 8 的已知差异：chrono 使用前推格里高利历，`java.util.Date` 在 1582-10-15
//! 之前使用儒略历；本模块只保证 1582-10-15 之后的日期与 JDK 8 一致。

use std::cmp::Ordering;

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Utc};
use jiff::tz::{AmbiguousOffset, Disambiguation, TimeZone};

use crate::error::RdbcError;

/// `java.sql.Timestamp` 的 JDK 8 语义扩展。
///
/// 对应 Java: `java.sql.Timestamp`。所有方法以 chrono 的 `NaiveDateTime` 作为载体，
/// 需要时区的入口按进程默认时区解释本地字段。
pub trait TimestampExt {
    /// 返回秒内的纳秒字段。
    ///
    /// 对应 Java: `java.sql.Timestamp#getNanos`。
    fn get_nanos(&self) -> u32;

    /// 替换秒内的纳秒字段，合法范围是 `0..=999999999`。
    ///
    /// 越界时返回 `InvalidArgument` 并且不修改原值，等价于 JDK 8 抛出的
    /// `IllegalArgumentException`。对应 Java: `java.sql.Timestamp#setNanos`。
    fn set_nanos(&mut self, nanos: i32) -> Result<(), RdbcError>;

    /// 输出 JDBC 时间戳文本 `yyyy-mm-dd hh:mm:ss.fffffffff`。
    ///
    /// 纳秒按 JDK 8 规则左补零到九位后裁掉末尾的零；纳秒为零时只输出 `0`。
    /// 对应 Java: `java.sql.Timestamp#toString`。
    fn to_jdbc_string(&self) -> String;

    /// 解析 JDBC 时间戳文本 `yyyy-mm-dd hh:mm:ss[.fffffffff]`。
    ///
    /// 月、日、时、分、秒允许省略前导零，文本前后允许空白；小数部分最多九位。
    /// 非法格式返回 `InvalidArgument`，等价于 JDK 8 抛出的 `IllegalArgumentException`。
    /// 对应 Java: `java.sql.Timestamp#valueOf(String)`。
    fn value_of(value: &str) -> Result<NaiveDateTime, RdbcError>;
    /// 由 `LocalDateTime` 构造 Timestamp。对应 Java: `Timestamp#valueOf(LocalDateTime)`。
    /// Java null 入参抛 NPE；Rust 签名以非空载体表达。
    fn value_of_local_date_time(value: NaiveDateTime) -> NaiveDateTime;

    /// 返回本地日期时间视图。
    ///
    /// 对应 Java: `java.sql.Timestamp#toLocalDateTime`。
    fn to_local_date_time(&self) -> NaiveDateTime;

    /// 返回按进程默认时区解释的瞬时值。
    ///
    /// 对应 Java: `java.sql.Timestamp#toInstant`。
    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError>;

    /// 由瞬时值按进程默认时区生成本地日期时间。
    ///
    /// 对应 Java: `java.sql.Timestamp#from(Instant)`。
    fn from_instant(instant: DateTime<Utc>) -> Result<NaiveDateTime, RdbcError>;

    /// 判断当前时间戳是否严格晚于 `other`。
    ///
    /// 先比较毫秒再比较纳秒。对应 Java: `java.sql.Timestamp#after`。
    fn after(&self, other: &NaiveDateTime) -> Result<bool, RdbcError>;

    /// 判断当前时间戳是否严格早于 `other`。
    ///
    /// 先比较毫秒再比较纳秒。对应 Java: `java.sql.Timestamp#before`。
    fn before(&self, other: &NaiveDateTime) -> Result<bool, RdbcError>;

    /// 与另一个时间戳比较，先比较毫秒再比较纳秒。
    ///
    /// 对应 Java: `java.sql.Timestamp#compareTo(Timestamp)`。
    fn compare_to(&self, other: &NaiveDateTime) -> Result<Ordering, RdbcError>;

    /// 与只精确到毫秒的日期比较，先比较毫秒再比较纳秒。
    ///
    /// `other` 被视为 `java.util.Date`，按 `new Timestamp(other.getTime())` 的规则
    /// 补齐纳秒字段。对应 Java: `java.sql.Timestamp#compareTo(java.util.Date)`。
    fn compare_to_date(&self, other: &NaiveDateTime) -> Result<Ordering, RdbcError>;

    /// 判断两个时间戳是否表示同一瞬时值。
    ///
    /// 对应 Java: `java.sql.Timestamp#equals(Timestamp)`。
    fn equals_timestamp(&self, other: &NaiveDateTime) -> Result<bool, RdbcError>;

    /// 判断两个时间戳是否表示同一瞬时值。
    ///
    /// 本 trait 只覆盖 `java.sql.Timestamp` 参数的场景，传入其它类型时 JDK 8 返回
    /// `false`。对应 Java: `java.sql.Timestamp#equals(Object)`。
    fn equals_object(&self, other: &NaiveDateTime) -> Result<bool, RdbcError>;

    /// 返回只由毫秒决定的散列值，不包含纳秒。
    ///
    /// 对应 Java: `java.sql.Timestamp#hashCode`。
    fn hash_code(&self) -> Result<i32, RdbcError>;

    /// 返回自 epoch 起的毫秒数，包含纳秒折算出的毫秒。
    ///
    /// 对应 Java: `java.sql.Timestamp#getTime`。
    fn get_time(&self) -> Result<i64, RdbcError>;

    /// 用自 epoch 起的毫秒数重置时间戳，精度截断到毫秒。
    ///
    /// 对应 Java: `java.sql.Timestamp#setTime(long)`。
    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError>;

    /// 按 JDK 8 的字段约定构造时间戳。
    ///
    /// `year` 以 1900 为基准，`month` 以 0 表示一月，这两点与
    /// `java.sql.Timestamp(int,...)` 一致；`day` 从 1 开始。越界的月、日、时、分、
    /// 秒按 JDK 8 宽松日历规则进位，`nanos` 超出 `0..=999999999` 返回 `InvalidArgument`。
    /// 对应 Java: `java.sql.Timestamp#<init>(int,int,int,int,int,int,int)`。
    fn new_timestamp(
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
        nanos: i32,
    ) -> Result<NaiveDateTime, RdbcError>;

    /// 按 epoch 毫秒构造时间戳，精度截断到毫秒。
    ///
    /// 对应 Java: `java.sql.Timestamp#<init>(long)`。
    fn new_timestamp_from_millis(millis: i64) -> Result<NaiveDateTime, RdbcError>;
}

/// `java.sql.Date` 的 JDK 8 语义扩展。
///
/// 对应 Java: `java.sql.Date`。所有方法以 chrono 的 `NaiveDate` 作为载体。
pub trait DateExt {
    /// 输出 JDBC 日期文本 `yyyy-mm-dd`。
    ///
    /// 对应 Java: `java.sql.Date#toString`。
    fn to_jdbc_string(&self) -> String;

    /// 解析 JDBC 日期文本 `yyyy-mm-dd`。
    ///
    /// 月、日允许省略前导零；非法格式返回 `InvalidArgument`，等价于 JDK 8 抛出的
    /// `IllegalArgumentException`。对应 Java: `java.sql.Date#valueOf(String)`。
    fn value_of(value: &str) -> Result<NaiveDate, RdbcError>;
    /// 由 `LocalDate` 构造 Date。对应 Java: `Date#valueOf(LocalDate)`。
    /// Java null 入参抛 NPE；Rust 签名以非空载体表达。
    fn value_of_local_date(value: NaiveDate) -> NaiveDate;

    /// 返回本地日期视图。
    ///
    /// 对应 Java: `java.sql.Date#toLocalDate`。
    fn to_local_date(&self) -> NaiveDate;

    /// JDK 8 的 `java.sql.Date` 没有时区语义，该入口恒定返回不支持。
    ///
    /// 对应 Java: `java.sql.Date#toInstant`，JDK 8 抛 `UnsupportedOperationException`。
    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError>;

    /// 用自 epoch 起的毫秒数重置日期。
    ///
    /// 对应 Java: `java.sql.Date#setTime(long)`。
    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError>;

    /// 按 JDK 8 的字段约定构造日期。
    ///
    /// `year` 以 1900 为基准，`month` 以 0 表示一月，`day` 从 1 开始；越界的月、日按
    /// JDK 8 宽松日历规则进位。对应 Java: `java.sql.Date#<init>(int,int,int)`。
    fn new_date(year: i32, month: i32, day: i32) -> Result<NaiveDate, RdbcError>;

    /// 按 epoch 毫秒构造日期，取进程默认时区下的本地日期。
    ///
    /// 对应 Java: `java.sql.Date#<init>(long)`。
    fn new_date_from_millis(millis: i64) -> Result<NaiveDate, RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#getHours`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_hours(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#getMinutes`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_minutes(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#getSeconds`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_seconds(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#setHours`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_hours(&mut self, hours: i32) -> Result<(), RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#setMinutes`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_minutes(&mut self, minutes: i32) -> Result<(), RdbcError>;

    /// JDK 8 的 `java.sql.Date` 丢弃时刻字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Date#setSeconds`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Date 在 JDK 8 中已废弃时刻修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_seconds(&mut self, seconds: i32) -> Result<(), RdbcError>;
}

/// `java.sql.Time` 的 JDK 8 语义扩展。
///
/// 对应 Java: `java.sql.Time`。所有方法以 chrono 的 `NaiveTime` 作为载体。
pub trait TimeExt {
    /// 输出 JDBC 时刻文本 `hh:mm:ss`。
    ///
    /// 对应 Java: `java.sql.Time#toString`。
    fn to_jdbc_string(&self) -> String;

    /// 解析 JDBC 时刻文本 `hh:mm:ss`。
    ///
    /// 时、分、秒允许省略前导零；非法格式返回 `InvalidArgument`，等价于 JDK 8 抛出的
    /// `IllegalArgumentException`。对应 Java: `java.sql.Time#valueOf(String)`。
    fn value_of(value: &str) -> Result<NaiveTime, RdbcError>;
    /// 由 `LocalTime` 构造 Time。对应 Java: `Time#valueOf(LocalTime)`。
    /// Java null 入参抛 NPE；Rust 签名以非空载体表达。
    fn value_of_local_time(value: NaiveTime) -> NaiveTime;

    /// 返回本地时刻视图。
    ///
    /// 对应 Java: `java.sql.Time#toLocalTime`。
    fn to_local_time(&self) -> NaiveTime;

    /// JDK 8 的 `java.sql.Time` 没有时区语义，该入口恒定返回不支持。
    ///
    /// 对应 Java: `java.sql.Time#toInstant`，JDK 8 抛 `UnsupportedOperationException`。
    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError>;

    /// 用自 epoch 起的毫秒数重置时刻。
    ///
    /// 对应 Java: `java.sql.Time#setTime(long)`。
    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError>;

    /// 按 JDK 8 的字段约定构造时刻，越界的时、分、秒按宽松日历规则进位。
    ///
    /// 对应 Java: `java.sql.Time#<init>(int,int,int)`。
    fn new_time(hour: i32, minute: i32, second: i32) -> Result<NaiveTime, RdbcError>;

    /// 按 epoch 毫秒构造时刻，取进程默认时区下的本地时刻。
    ///
    /// 对应 Java: `java.sql.Time#<init>(long)`。
    fn new_time_from_millis(millis: i64) -> Result<NaiveTime, RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#getYear`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_year(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#getMonth`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_month(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#getDay`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_day(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#getDate`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期访问器，调用恒抛 IllegalArgumentException"
    )]
    fn get_date(&self) -> Result<i32, RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#setYear`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_year(&mut self, year: i32) -> Result<(), RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#setMonth`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_month(&mut self, month: i32) -> Result<(), RdbcError>;

    /// JDK 8 的 `java.sql.Time` 丢弃日期字段，该入口恒定返回参数错误。
    ///
    /// 对应 Java: `java.sql.Time#setDate`，JDK 8 抛 `IllegalArgumentException`。
    #[deprecated(
        note = "java.sql.Time 在 JDK 8 中已废弃日期修改器，调用恒抛 IllegalArgumentException"
    )]
    fn set_date(&mut self, date: i32) -> Result<(), RdbcError>;
}

impl TimestampExt for NaiveDateTime {
    fn get_nanos(&self) -> u32 {
        self.nanosecond()
    }

    fn set_nanos(&mut self, nanos: i32) -> Result<(), RdbcError> {
        let nanos = u32::try_from(nanos)
            .ok()
            .filter(|value| *value <= 999_999_999)
            .ok_or_else(|| {
                RdbcError::InvalidArgument(format!(
                    "nanos must be between 0 and 999999999: {nanos}"
                ))
            })?;
        *self = self.with_nanosecond(nanos).ok_or_else(|| {
            RdbcError::InvalidArgument(format!("nanos out of range for timestamp: {nanos}"))
        })?;
        Ok(())
    }

    fn to_jdbc_string(&self) -> String {
        let nanos = self.nanosecond();
        // JDK 8 的 Timestamp#toString 会补零到九位后裁掉末尾的零；纳秒为零时只输出 "0"。
        let fraction = if nanos == 0 {
            "0".to_owned()
        } else {
            format!("{nanos:09}").trim_end_matches('0').to_owned()
        };
        format!(
            "{}-{:02}-{:02} {:02}:{:02}:{:02}.{}",
            java_year_text(self.year()),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second(),
            fraction
        )
    }

    fn value_of(value: &str) -> Result<NaiveDateTime, RdbcError> {
        parse_jdbc_timestamp(value)
    }

    fn value_of_local_date_time(value: NaiveDateTime) -> NaiveDateTime {
        value
    }

    fn to_local_date_time(&self) -> NaiveDateTime {
        *self
    }

    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError> {
        let (seconds, nanos) = split_instant(local_to_instant(*self)?);
        DateTime::from_timestamp(seconds, nanos).ok_or_else(|| {
            RdbcError::InvalidArgument(format!("timestamp instant is out of range: {self}"))
        })
    }

    fn from_instant(instant: DateTime<Utc>) -> Result<NaiveDateTime, RdbcError> {
        let jiff_instant = jiff::Timestamp::new(
            instant.timestamp(),
            i32::try_from(instant.timestamp_subsec_nanos()).unwrap_or(i32::MAX),
        )
        .map_err(|error| {
            RdbcError::InvalidArgument(format!("invalid timestamp instant: {error}"))
        })?;
        instant_to_local(jiff_instant)
    }

    fn after(&self, other: &NaiveDateTime) -> Result<bool, RdbcError> {
        Ok(self.compare_to(other)? == Ordering::Greater)
    }

    fn before(&self, other: &NaiveDateTime) -> Result<bool, RdbcError> {
        Ok(self.compare_to(other)? == Ordering::Less)
    }

    fn compare_to(&self, other: &NaiveDateTime) -> Result<Ordering, RdbcError> {
        let (this_seconds, this_nanos) = split_instant(local_to_instant(*self)?);
        let (that_seconds, that_nanos) = split_instant(local_to_instant(*other)?);
        Ok(compare_millis_then_nanos(
            java_epoch_millis(this_seconds, this_nanos)?,
            this_nanos,
            java_epoch_millis(that_seconds, that_nanos)?,
            that_nanos,
        ))
    }

    fn compare_to_date(&self, other: &NaiveDateTime) -> Result<Ordering, RdbcError> {
        let (this_seconds, this_nanos) = split_instant(local_to_instant(*self)?);
        let (that_seconds, that_nanos) = split_instant(local_to_instant(*other)?);
        let that_millis = java_epoch_millis(that_seconds, that_nanos)?;
        // java.sql.Timestamp(long) 把毫秒余数放进纳秒字段，负值按 Java 的取整规则回绕。
        let that_nanos =
            u32::try_from(((that_millis % 1_000 + 1_000) % 1_000) * 1_000_000).unwrap_or_default();
        Ok(compare_millis_then_nanos(
            java_epoch_millis(this_seconds, this_nanos)?,
            this_nanos,
            that_millis,
            that_nanos,
        ))
    }

    fn equals_timestamp(&self, other: &NaiveDateTime) -> Result<bool, RdbcError> {
        let (this_seconds, this_nanos) = split_instant(local_to_instant(*self)?);
        let (that_seconds, that_nanos) = split_instant(local_to_instant(*other)?);
        Ok(java_epoch_millis(this_seconds, this_nanos)?
            == java_epoch_millis(that_seconds, that_nanos)?
            && this_nanos == that_nanos)
    }

    fn equals_object(&self, other: &NaiveDateTime) -> Result<bool, RdbcError> {
        self.equals_timestamp(other)
    }

    fn hash_code(&self) -> Result<i32, RdbcError> {
        let millis = self.get_time()?;
        let bits = millis as u64;
        Ok((bits ^ (bits >> 32)) as i32)
    }

    fn get_time(&self) -> Result<i64, RdbcError> {
        let (seconds, nanos) = split_instant(local_to_instant(*self)?);
        java_epoch_millis(seconds, nanos)
    }

    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError> {
        *self = instant_to_local(millis_to_instant(millis)?)?;
        Ok(())
    }

    fn new_timestamp(
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
        nanos: i32,
    ) -> Result<NaiveDateTime, RdbcError> {
        if !(0..=999_999_999).contains(&nanos) {
            return Err(RdbcError::InvalidArgument(format!(
                "nanos must be between 0 and 999999999: {nanos}"
            )));
        }
        // java.sql.Timestamp(int,...) 的 year 以 1900 为基准，month 以 0 表示一月。
        normalize_java_datetime(
            i64::from(year) + 1_900,
            month,
            day,
            hour,
            minute,
            second,
            nanos as u32,
        )
    }

    fn new_timestamp_from_millis(millis: i64) -> Result<NaiveDateTime, RdbcError> {
        instant_to_local(millis_to_instant(millis)?)
    }
}

impl DateExt for NaiveDate {
    fn to_jdbc_string(&self) -> String {
        format!(
            "{}-{:02}-{:02}",
            java_year_text(self.year()),
            self.month(),
            self.day()
        )
    }

    fn value_of(value: &str) -> Result<NaiveDate, RdbcError> {
        parse_jdbc_date(value)
    }

    fn value_of_local_date(value: NaiveDate) -> NaiveDate {
        value
    }

    fn to_local_date(&self) -> NaiveDate {
        *self
    }

    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "java.sql.Date#toInstant",
        })
    }

    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError> {
        *self = instant_to_local(millis_to_instant(millis)?)?.date();
        Ok(())
    }

    fn new_date(year: i32, month: i32, day: i32) -> Result<NaiveDate, RdbcError> {
        // java.sql.Date(int,int,int) 的 year 以 1900 为基准，month 以 0 表示一月。
        normalize_java_datetime(i64::from(year) + 1_900, month, day, 0, 0, 0, 0)
            .map(|value| value.date())
    }

    fn new_date_from_millis(millis: i64) -> Result<NaiveDate, RdbcError> {
        Ok(instant_to_local(millis_to_instant(millis)?)?.date())
    }

    fn get_hours(&self) -> Result<i32, RdbcError> {
        Err(java_sql_date_time_field_error("getHours"))
    }

    fn get_minutes(&self) -> Result<i32, RdbcError> {
        Err(java_sql_date_time_field_error("getMinutes"))
    }

    fn get_seconds(&self) -> Result<i32, RdbcError> {
        Err(java_sql_date_time_field_error("getSeconds"))
    }

    fn set_hours(&mut self, _hours: i32) -> Result<(), RdbcError> {
        Err(java_sql_date_time_field_error("setHours"))
    }

    fn set_minutes(&mut self, _minutes: i32) -> Result<(), RdbcError> {
        Err(java_sql_date_time_field_error("setMinutes"))
    }

    fn set_seconds(&mut self, _seconds: i32) -> Result<(), RdbcError> {
        Err(java_sql_date_time_field_error("setSeconds"))
    }
}

impl TimeExt for NaiveTime {
    fn to_jdbc_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )
    }

    fn value_of(value: &str) -> Result<NaiveTime, RdbcError> {
        parse_jdbc_time(value)
    }

    fn value_of_local_time(value: NaiveTime) -> NaiveTime {
        value
    }

    fn to_local_time(&self) -> NaiveTime {
        *self
    }

    fn to_instant(&self) -> Result<DateTime<Utc>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "java.sql.Time#toInstant",
        })
    }

    fn set_time(&mut self, millis: i64) -> Result<(), RdbcError> {
        *self = instant_to_local(millis_to_instant(millis)?)?.time();
        Ok(())
    }

    fn new_time(hour: i32, minute: i32, second: i32) -> Result<NaiveTime, RdbcError> {
        let total = i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
        let remainder = total.rem_euclid(86_400);
        NaiveTime::from_hms_nano_opt(
            (remainder / 3_600) as u32,
            ((remainder % 3_600) / 60) as u32,
            (remainder % 60) as u32,
            0,
        )
        .ok_or_else(|| {
            RdbcError::InvalidArgument(format!(
                "java.sql.Time fields are out of range: {hour}:{minute}:{second}"
            ))
        })
    }

    fn new_time_from_millis(millis: i64) -> Result<NaiveTime, RdbcError> {
        Ok(instant_to_local(millis_to_instant(millis)?)?.time())
    }

    fn get_year(&self) -> Result<i32, RdbcError> {
        Err(java_sql_time_date_field_error("getYear"))
    }

    fn get_month(&self) -> Result<i32, RdbcError> {
        Err(java_sql_time_date_field_error("getMonth"))
    }

    fn get_day(&self) -> Result<i32, RdbcError> {
        Err(java_sql_time_date_field_error("getDay"))
    }

    fn get_date(&self) -> Result<i32, RdbcError> {
        Err(java_sql_time_date_field_error("getDate"))
    }

    fn set_year(&mut self, _year: i32) -> Result<(), RdbcError> {
        Err(java_sql_time_date_field_error("setYear"))
    }

    fn set_month(&mut self, _month: i32) -> Result<(), RdbcError> {
        Err(java_sql_time_date_field_error("setMonth"))
    }

    fn set_date(&mut self, _date: i32) -> Result<(), RdbcError> {
        Err(java_sql_time_date_field_error("setDate"))
    }
}

/// 解析进程默认时区。
///
/// 优先使用 `TZ` 环境变量，未设置时回落到系统时区，与 `druid_rdbc::types::Timestamp`
/// 的时区解析保持一致。
fn default_time_zone() -> Result<TimeZone, RdbcError> {
    if let Some(zone_name) = std::env::var_os("TZ") {
        let zone_name = zone_name.to_string_lossy();
        TimeZone::get(zone_name.as_ref()).map_err(|error| {
            RdbcError::InvalidArgument(format!("invalid process time-zone setting: {error}"))
        })
    } else {
        Ok(TimeZone::system())
    }
}

/// 把本地日期时间按进程默认时区解释为瞬时值。
///
/// DST 空洞按 Java 宽松日历前移，重叠时取较晚的时刻，与 JDK 8 的 `Calendar` 一致。
fn local_to_instant(value: NaiveDateTime) -> Result<jiff::Timestamp, RdbcError> {
    let civil = jiff::civil::DateTime::new(
        i16::try_from(value.year()).map_err(|_| local_range_error(value))?,
        value.month() as i8,
        value.day() as i8,
        value.hour() as i8,
        value.minute() as i8,
        value.second() as i8,
        value.nanosecond() as i32,
    )
    .map_err(|_| local_range_error(value))?;
    let zone = default_time_zone()?;
    let ambiguous = zone.to_ambiguous_zoned(civil);
    let disambiguation = match ambiguous.offset() {
        AmbiguousOffset::Fold { .. } => Disambiguation::Later,
        AmbiguousOffset::Gap { .. } | AmbiguousOffset::Unambiguous { .. } => {
            Disambiguation::Compatible
        }
    };
    ambiguous
        .disambiguate(disambiguation)
        .map(|zoned| zoned.timestamp())
        .map_err(|_| local_range_error(value))
}

/// 把瞬时值转换为进程默认时区下的本地日期时间。
fn instant_to_local(instant: jiff::Timestamp) -> Result<NaiveDateTime, RdbcError> {
    let zone = default_time_zone()?;
    jiff_to_chrono(instant.to_zoned(zone).datetime())
}

/// 把 jiff 的民用日期时间转换为 chrono 的本地日期时间。
fn jiff_to_chrono(value: jiff::civil::DateTime) -> Result<NaiveDateTime, RdbcError> {
    NaiveDate::from_ymd_opt(
        i32::from(value.year()),
        value.month() as u32,
        value.day() as u32,
    )
    .and_then(|date| {
        date.and_hms_nano_opt(
            value.hour() as u32,
            value.minute() as u32,
            value.second() as u32,
            value.subsec_nanosecond() as u32,
        )
    })
    .ok_or_else(|| {
        RdbcError::InvalidArgument(format!("JDBC TIMESTAMP is outside Chrono range: {value}"))
    })
}

/// 把 epoch 毫秒转换为瞬时值，精度截断到毫秒。
fn millis_to_instant(millis: i64) -> Result<jiff::Timestamp, RdbcError> {
    let seconds = millis.div_euclid(1_000);
    let nanos = i32::try_from(millis.rem_euclid(1_000) * 1_000_000).unwrap_or(i32::MAX);
    jiff::Timestamp::new(seconds, nanos).map_err(|error| {
        RdbcError::InvalidArgument(format!("invalid SQL TIMESTAMP millis {millis}: {error}"))
    })
}

/// 把瞬时值拆分为 epoch 秒与秒内纳秒。
///
/// jiff 的 `as_second` 与 `subsec_nanosecond` 采用向零截断的有符号分解，
/// 负 epoch 秒必须归一化到 `epoch_second + 纳秒余数`。
fn split_instant(instant: jiff::Timestamp) -> (i64, u32) {
    let seconds = instant.as_second();
    let subsec = instant.subsec_nanosecond();
    if subsec < 0 {
        (seconds - 1, (subsec + 1_000_000_000) as u32)
    } else {
        (seconds, subsec as u32)
    }
}

/// 按 JDK 8 的 `Timestamp#getTime` 语义计算 epoch 毫秒。
fn java_epoch_millis(seconds: i64, nanos: u32) -> Result<i64, RdbcError> {
    seconds
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(i64::from(nanos / 1_000_000)))
        .ok_or_else(|| RdbcError::InvalidArgument("SQL TIMESTAMP millis overflow".to_owned()))
}

/// 生成时区转换失败时的参数错误。
fn local_range_error(value: NaiveDateTime) -> RdbcError {
    RdbcError::InvalidArgument(format!(
        "local TIMESTAMP is outside the platform time-zone range: {value}"
    ))
}

/// 按 JDK 8 的 `Integer.parseInt` 语义解析十进制整数。
///
/// 允许前导 `+`/`-`，不允许空白与非 ASCII 数字，超出 `int` 范围视为解析失败。
fn java_parse_int(text: &str) -> Result<i32, RdbcError> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RdbcError::InvalidArgument(format!(
            "invalid java integer: '{text}'"
        )));
    }
    let magnitude = digits
        .parse::<i64>()
        .map_err(|_| RdbcError::InvalidArgument(format!("java integer overflow: '{text}'")))?;
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value)
        .map_err(|_| RdbcError::InvalidArgument(format!("java integer overflow: '{text}'")))
}

/// 按 JDK 8 宽松日历规则归一化日期时间字段。
///
/// `year` 是最终年份，`month` 以 0 表示一月。越界的月、日、时、分、秒按进位处理，
/// 与 JDK 8 的 `java.util.Date` 构造器一致。
fn normalize_java_datetime(
    year: i64,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    nanos: u32,
) -> Result<NaiveDateTime, RdbcError> {
    let year = year + i64::from(month).div_euclid(12);
    let month = i64::from(month).rem_euclid(12) + 1;
    let year = i32::try_from(year).map_err(|_| {
        RdbcError::InvalidArgument(format!("java date year is out of range: {year}"))
    })?;
    let month = u32::try_from(month).unwrap_or(1);
    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        RdbcError::InvalidArgument(format!("java date is out of range: {year}-{month}"))
    })?;
    let days = first_of_month
        .num_days_from_ce()
        .checked_add(day - 1)
        .ok_or_else(|| RdbcError::InvalidArgument("java date day offset overflow".to_owned()))?;
    let date = NaiveDate::from_num_days_from_ce_opt(days).ok_or_else(|| {
        RdbcError::InvalidArgument(format!("java date is out of range: {year}-{month}"))
    })?;

    let total_seconds = i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
    let day_shift = total_seconds.div_euclid(86_400);
    let remainder = total_seconds.rem_euclid(86_400);
    let date = date
        .checked_add_signed(TimeDelta::days(day_shift))
        .ok_or_else(|| RdbcError::InvalidArgument("java date time overflow".to_owned()))?;
    date.and_hms_nano_opt(
        (remainder / 3_600) as u32,
        ((remainder % 3_600) / 60) as u32,
        (remainder % 60) as u32,
        nanos,
    )
    .ok_or_else(|| RdbcError::InvalidArgument("java date time is out of range".to_owned()))
}

/// 输出 JDK 8 的四位年份文本。
///
/// JDK 8 在年份小于 1000 时补齐四位，其它年份直接输出十进制文本。
fn java_year_text(year: i32) -> String {
    if (0..1_000).contains(&year) {
        format!("{year:04}")
    } else {
        year.to_string()
    }
}

/// 按 JDK 8 的 `Timestamp#compareTo` 规则先比较毫秒再比较纳秒。
fn compare_millis_then_nanos(
    this_millis: i64,
    this_nanos: u32,
    that_millis: i64,
    that_nanos: u32,
) -> Ordering {
    match this_millis.cmp(&that_millis) {
        Ordering::Equal => this_nanos.cmp(&that_nanos),
        ordering => ordering,
    }
}

/// 生成 `java.sql.Date` 废弃时刻访问器的参数错误。
fn java_sql_date_time_field_error(name: &str) -> RdbcError {
    RdbcError::InvalidArgument(format!(
        "java.sql.Date#{name} is not supported: java.sql.Date has no time-of-day fields"
    ))
}

/// 生成 `java.sql.Time` 废弃日期访问器的参数错误。
fn java_sql_time_date_field_error(name: &str) -> RdbcError {
    RdbcError::InvalidArgument(format!(
        "java.sql.Time#{name} is not supported: java.sql.Time has no date fields"
    ))
}

/// 按 JDK 8 的 `java.sql.Timestamp#valueOf(String)` 解析时间戳文本。
fn parse_jdbc_timestamp(value: &str) -> Result<NaiveDateTime, RdbcError> {
    const FORMAT_ERROR: &str = "Timestamp format must be yyyy-mm-dd hh:mm:ss[.fffffffff]";
    let trimmed = value.trim();
    let Some(space) = trimmed.find(' ') else {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    };
    if space == 0 {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    }
    let date_text = &trimmed[..space];
    let time_text = &trimmed[space + 1..];

    let Some(first_dash) = date_text.find('-') else {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    };
    let Some(second_dash) = date_text[first_dash + 1..].find('-') else {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    };
    let second_dash = first_dash + 1 + second_dash;
    if first_dash == 0 || second_dash == 0 || second_dash + 1 >= date_text.len() {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    }
    let year_text = &date_text[..first_dash];
    let month_text = &date_text[first_dash + 1..second_dash];
    let day_text = &date_text[second_dash + 1..];
    if year_text.len() != 4
        || !(1..=2).contains(&month_text.len())
        || !(1..=2).contains(&day_text.len())
    {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    }
    let year = java_parse_int(year_text)?;
    let month = java_parse_int(month_text)?;
    let day = java_parse_int(day_text)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    }

    let Some(first_colon) = time_text.find(':') else {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    };
    let Some(second_colon) = time_text[first_colon + 1..].find(':') else {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    };
    let second_colon = first_colon + 1 + second_colon;
    if first_colon == 0 || second_colon == 0 || second_colon + 1 >= time_text.len() {
        return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
    }
    let hour = java_parse_int(&time_text[..first_colon])?;
    let minute = java_parse_int(&time_text[first_colon + 1..second_colon])?;
    let period = time_text[second_colon + 1..]
        .find('.')
        .map(|index| second_colon + 1 + index);

    let (second, nanos) = match period {
        Some(period) if period > second_colon + 1 && period + 1 < time_text.len() => {
            let second = java_parse_int(&time_text[second_colon + 1..period])?;
            let fraction = &time_text[period + 1..];
            if fraction.len() > 9 {
                return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
            }
            if !fraction.starts_with(|character: char| character.is_ascii_digit()) {
                return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned()));
            }
            // JDK 8 把小数部分右补零到九位。
            let padded = format!("{fraction:0<9}");
            (second, java_parse_int(&padded)?)
        }
        Some(_) => return Err(RdbcError::InvalidArgument(FORMAT_ERROR.to_owned())),
        None => (java_parse_int(&time_text[second_colon + 1..])?, 0),
    };

    normalize_java_datetime(
        i64::from(year),
        month - 1,
        day,
        hour,
        minute,
        second,
        nanos as u32,
    )
}

/// 按 JDK 8 的 `java.sql.Date#valueOf(String)` 解析日期文本。
fn parse_jdbc_date(value: &str) -> Result<NaiveDate, RdbcError> {
    let Some(first_dash) = value.find('-') else {
        return Err(jdbc_date_format_error(value));
    };
    let Some(second_dash) = value[first_dash + 1..].find('-') else {
        return Err(jdbc_date_format_error(value));
    };
    let second_dash = first_dash + 1 + second_dash;
    if first_dash == 0 || second_dash == 0 || second_dash + 1 >= value.len() {
        return Err(jdbc_date_format_error(value));
    }
    let year_text = &value[..first_dash];
    let month_text = &value[first_dash + 1..second_dash];
    let day_text = &value[second_dash + 1..];
    if year_text.len() != 4
        || !(1..=2).contains(&month_text.len())
        || !(1..=2).contains(&day_text.len())
    {
        return Err(jdbc_date_format_error(value));
    }
    let year = java_parse_int(year_text)?;
    let month = java_parse_int(month_text)?;
    let day = java_parse_int(day_text)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(jdbc_date_format_error(value));
    }
    normalize_java_datetime(i64::from(year), month - 1, day, 0, 0, 0, 0).map(|value| value.date())
}

/// 按 JDK 8 的 `java.sql.Time#valueOf(String)` 解析时刻文本。
fn parse_jdbc_time(value: &str) -> Result<NaiveTime, RdbcError> {
    let Some(first_colon) = value.find(':') else {
        return Err(jdbc_time_format_error(value));
    };
    let Some(second_colon) = value[first_colon + 1..].find(':') else {
        return Err(jdbc_time_format_error(value));
    };
    let second_colon = first_colon + 1 + second_colon;
    if first_colon == 0 || second_colon == 0 || second_colon + 1 >= value.len() {
        return Err(jdbc_time_format_error(value));
    }
    let hour = java_parse_int(&value[..first_colon])?;
    let minute = java_parse_int(&value[first_colon + 1..second_colon])?;
    let second = java_parse_int(&value[second_colon + 1..])?;
    let total = i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
    let remainder = total.rem_euclid(86_400);
    NaiveTime::from_hms_nano_opt(
        (remainder / 3_600) as u32,
        ((remainder % 3_600) / 60) as u32,
        (remainder % 60) as u32,
        0,
    )
    .ok_or_else(|| jdbc_time_format_error(value))
}

/// 生成 `java.sql.Date#valueOf(String)` 的格式错误。
fn jdbc_date_format_error(value: &str) -> RdbcError {
    RdbcError::InvalidArgument(format!("invalid java.sql.Date value: '{value}'"))
}

/// 生成 `java.sql.Time#valueOf(String)` 的格式错误。
fn jdbc_time_format_error(value: &str) -> RdbcError {
    RdbcError::InvalidArgument(format!("invalid java.sql.Time value: '{value}'"))
}
