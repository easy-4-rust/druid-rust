use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use jiff::tz::{AmbiguousOffset, Disambiguation, TimeZone};
use std::hash::{Hash, Hasher};

use crate::error::RdbcError;
use crate::types::Value;

/// JDBC TIMESTAMP 时刻及其纳秒精度。
///
/// 对应 Java: `java.sql.Timestamp`。绝对秒与纳秒作为身份值保存；本地字段按调用时的进程默认时区计算。
#[derive(Clone, Copy, Debug)]
pub struct Timestamp {
    epoch_seconds: i64,
    nanos: u32,
    local_cache: NaiveDateTime,
}

impl Timestamp {
    /// 从 epoch 毫秒按进程默认时区创建时间戳。对应 Java: `Timestamp(long)`。
    pub fn from_millis(millis: i64) -> Result<Self, RdbcError> {
        let epoch_seconds = millis.div_euclid(1_000);
        let nanos = (millis.rem_euclid(1_000) as u32) * 1_000_000;
        Self::from_instant(epoch_seconds, nanos)
    }

    /// 解析 JDBC TIMESTAMP 转义格式，并按进程默认时区解释本地字段。对应 Java: `valueOf(String)`。
    pub fn value_of(value: &str) -> Result<Self, RdbcError> {
        let local =
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f").map_err(|error| {
                RdbcError::InvalidArgument(format!("invalid RDBC TIMESTAMP '{value}': {error}"))
            })?;
        Self::from_local_date_time(local)
    }

    /// 从本地日期时间字段创建时间戳；DST gap 前移、overlap 选较晚时刻，对齐 Java 8 Calendar。
    pub fn from_local_date_time(value: NaiveDateTime) -> Result<Self, RdbcError> {
        let instant = local_to_instant(value)?;
        Self::from_instant(instant.as_second(), instant.subsec_nanosecond() as u32)
    }

    fn from_instant(epoch_seconds: i64, nanos: u32) -> Result<Self, RdbcError> {
        let instant = jiff::Timestamp::new(epoch_seconds, nanos as i32).map_err(|error| {
            RdbcError::InvalidArgument(format!("invalid SQL TIMESTAMP instant: {error}"))
        })?;
        let zoned = instant.to_zoned(default_time_zone()?);
        let local_cache = chrono_from_jiff(zoned.datetime())?;
        Ok(Self {
            epoch_seconds,
            nanos,
            local_cache,
        })
    }

    /// 返回按当前进程默认时区计算的本地字段。对应 Java: `toLocalDateTime`。
    #[must_use]
    pub fn to_local_date_time(self) -> NaiveDateTime {
        default_time_zone()
            .ok()
            .and_then(|zone| {
                jiff::Timestamp::new(self.epoch_seconds, self.nanos as i32)
                    .ok()
                    .map(|instant| chrono_from_jiff(instant.to_zoned(zone).datetime()))
            })
            .and_then(Result::ok)
            .unwrap_or(self.local_cache)
    }

    /// 返回纳秒字段。对应 Java: `getNanos`。
    #[must_use]
    pub fn nanos(self) -> u32 {
        self.nanos
    }

    /// 替换纳秒字段；非法值失败且保留原对象。对应 Java: `Timestamp#setNanos`。
    pub fn set_nanos(&mut self, nanos: u32) -> Result<(), RdbcError> {
        if nanos > 999_999_999 {
            return Err(RdbcError::InvalidArgument(format!(
                "nanos must be between 0 and 999999999: {nanos}"
            )));
        }
        let updated = Self::from_instant(self.epoch_seconds, nanos)?;
        *self = updated;
        Ok(())
    }

    /// 返回当前瞬间的 epoch 毫秒；超出 i64 毫秒范围时返回参数错误。
    pub fn millis(self) -> Result<i64, RdbcError> {
        self.epoch_seconds
            .checked_mul(1_000)
            .and_then(|seconds| seconds.checked_add(i64::from(self.nanos / 1_000_000)))
            .ok_or_else(|| RdbcError::InvalidArgument("SQL TIMESTAMP millis overflow".into()))
    }
}

fn local_to_instant(value: NaiveDateTime) -> Result<jiff::Timestamp, RdbcError> {
    let datetime = jiff::civil::DateTime::new(
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
    let ambiguous = zone.to_ambiguous_zoned(datetime);
    // Java lenient Calendar moves forward across gaps, while overlaps choose the later instant.
    let disambiguation = match ambiguous.offset() {
        AmbiguousOffset::Gap { .. } => Disambiguation::Compatible,
        AmbiguousOffset::Fold { .. } => Disambiguation::Later,
        AmbiguousOffset::Unambiguous { .. } => Disambiguation::Compatible,
    };
    ambiguous
        .disambiguate(disambiguation)
        .map(|value| value.timestamp())
        .map_err(|_| local_range_error(value))
}

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

fn local_range_error(value: NaiveDateTime) -> RdbcError {
    RdbcError::InvalidArgument(format!(
        "local TIMESTAMP is outside the platform time-zone range: {value}"
    ))
}

fn chrono_from_jiff(value: jiff::civil::DateTime) -> Result<NaiveDateTime, RdbcError> {
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

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        self.epoch_seconds == other.epoch_seconds && self.nanos == other.nanos
    }
}
impl Eq for Timestamp {}
impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.epoch_seconds, self.nanos).cmp(&(other.epoch_seconds, other.nanos))
    }
}
impl Hash for Timestamp {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.epoch_seconds.hash(state);
        self.nanos.hash(state);
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}",
            self.to_local_date_time().format("%Y-%m-%d %H:%M:%S")
        )?;
        let fraction = format!("{:09}", self.nanos);
        let fraction = fraction.trim_end_matches('0');
        write!(
            formatter,
            ".{}",
            if fraction.is_empty() { "0" } else { fraction }
        )
    }
}

impl From<Timestamp> for Value {
    fn from(value: Timestamp) -> Self {
        Self::Timestamp(value.to_local_date_time())
    }
}
