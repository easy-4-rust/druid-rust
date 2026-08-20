use chrono::{Local, NaiveDate, TimeZone};

use crate::core::{DruidError, Value};

/// Thin date wrapper that lets RDBC identify an SQL `DATE` value.
///
/// Corresponds to Java: `java.sql.Date`. It retains only year, month, and day. Construction
/// from epoch milliseconds uses the local time zone and discards the time component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(NaiveDate);

impl Date {
    /// Normalizes epoch `millis` to SQL `DATE` in the local time zone.
    ///
    /// Returns `InvalidArgument` when the local representation is not unique.
    pub fn from_millis(millis: i64) -> Result<Self, DruidError> {
        Local
            .timestamp_millis_opt(millis)
            .single()
            .map(|value| Self(value.date_naive()))
            .ok_or_else(|| {
                DruidError::InvalidArgument(format!(
                    "invalid or ambiguous SQL DATE millis: {millis}"
                ))
            })
    }

    /// Parses the RDBC date escape format `yyyy-[m]m-[d]d`.
    ///
    /// Invalid syntax or fields return `InvalidArgument`. Corresponds to Java:
    /// `Date#valueOf(String)`.
    pub fn value_of(value: &str) -> Result<Self, DruidError> {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Self)
            .map_err(|error| {
                DruidError::InvalidArgument(format!("invalid RDBC DATE '{value}': {error}"))
            })
    }

    /// Creates the same SQL `DATE` from `value`; corresponds to Java `valueOf(LocalDate)`.
    #[must_use]
    pub fn from_local_date(value: NaiveDate) -> Self {
        Self(value)
    }

    /// Returns the local date without a time zone. Corresponds to Java: `toLocalDate`.
    #[must_use]
    pub fn to_local_date(self) -> NaiveDate {
        self.0
    }

    /// Returns epoch milliseconds for local midnight on this date.
    ///
    /// An ambiguous local midnight returns `InvalidArgument`. Corresponds to inherited `getTime`.
    pub fn millis(self) -> Result<i64, DruidError> {
        let local = self
            .0
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid");
        Local
            .from_local_datetime(&local)
            .single()
            .map(|value| value.timestamp_millis())
            .ok_or_else(|| {
                DruidError::InvalidArgument(format!(
                    "ambiguous local midnight for SQL DATE {}",
                    self.0
                ))
            })
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.format("%Y-%m-%d"))
    }
}

impl From<Date> for Value {
    fn from(value: Date) -> Self {
        Self::Date(value.0)
    }
}
