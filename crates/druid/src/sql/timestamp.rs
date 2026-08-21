use chrono::{DateTime, NaiveDateTime, Timelike, Utc};

use crate::core::{DruidError, Value};

/// Thin date-time wrapper that lets RDBC identify an SQL `TIMESTAMP` value.
///
/// Corresponds to Java: `java.sql.Timestamp`. `NaiveDateTime` preserves the zone-free fields;
/// the fractional second participates in comparison and formatting at nanosecond precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(NaiveDateTime);

impl Timestamp {
    /// Creates a timestamp from epoch `millis` on the UTC timeline.
    ///
    /// Out-of-range values return `InvalidArgument`. Corresponds to Java: `Timestamp(long)`.
    pub fn from_millis(millis: i64) -> Result<Self, DruidError> {
        DateTime::<Utc>::from_timestamp_millis(millis)
            .map(|value| Self(value.naive_utc()))
            .ok_or_else(|| {
                DruidError::InvalidArgument(format!("invalid SQL TIMESTAMP millis: {millis}"))
            })
    }

    /// Parses the RDBC timestamp escape format `yyyy-[m]m-[d]d hh:mm:ss[.f...]`.
    ///
    /// Invalid syntax or fields return `InvalidArgument`. Corresponds to Java: `valueOf(String)`.
    pub fn value_of(value: &str) -> Result<Self, DruidError> {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
            .map(Self)
            .map_err(|error| {
                DruidError::InvalidArgument(format!("invalid RDBC TIMESTAMP '{value}': {error}"))
            })
    }

    /// Creates the same SQL timestamp from `value`; corresponds to `valueOf(LocalDateTime)`.
    #[must_use]
    pub fn from_local_date_time(value: NaiveDateTime) -> Self {
        Self(value)
    }

    /// Returns the local date-time fields. Corresponds to Java: `toLocalDateTime`.
    #[must_use]
    pub fn to_local_date_time(self) -> NaiveDateTime {
        self.0
    }

    /// Returns the fractional-second nanoseconds. Corresponds to Java: `getNanos`.
    #[must_use]
    pub fn nanos(self) -> u32 {
        self.0.and_utc().timestamp_subsec_nanos()
    }

    /// Replaces the fractional second with `nanos`.
    ///
    /// The value must be in 0..=999,999,999; otherwise `InvalidArgument` is returned.
    /// Corresponds to Java: `Timestamp#setNanos`.
    pub fn set_nanos(&mut self, nanos: u32) -> Result<(), DruidError> {
        if nanos > 999_999_999 {
            return Err(DruidError::InvalidArgument(format!(
                "nanos must be between 0 and 999999999: {nanos}"
            )));
        }
        self.0 = self.0.with_nanosecond(nanos).ok_or_else(|| {
            DruidError::InvalidArgument(format!("nanos must be between 0 and 999999999: {nanos}"))
        })?;
        Ok(())
    }

    /// Returns epoch milliseconds, interpreting zone-free fields on the UTC timeline.
    #[must_use]
    pub fn millis(self) -> i64 {
        self.0.and_utc().timestamp_millis()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.format("%Y-%m-%d %H:%M:%S%.f"))
    }
}

impl From<Timestamp> for Value {
    fn from(value: Timestamp) -> Self {
        Self::Timestamp(value.0)
    }
}
