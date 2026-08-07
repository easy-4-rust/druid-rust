use chrono::NaiveTime;

use crate::core::{DruidError, Value};

/// Thin time wrapper that lets RDBC identify an SQL `TIME` value.
///
/// Corresponds to Java: `java.sql.Time`. It represents hour, minute, and second without a date
/// or time zone and formats with the RDBC time escape form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(NaiveTime);

impl Time {
    /// Parses the RDBC time escape format `hh:mm:ss`.
    ///
    /// Invalid syntax or fields return `InvalidArgument`. Corresponds to Java:
    /// `Time#valueOf(String)`.
    pub fn value_of(value: &str) -> Result<Self, DruidError> {
        NaiveTime::parse_from_str(value, "%H:%M:%S")
            .map(Self)
            .map_err(|error| {
                DruidError::InvalidArgument(format!("invalid RDBC TIME '{value}': {error}"))
            })
    }

    /// Creates the same SQL `TIME` from `value`; corresponds to Java `valueOf(LocalTime)`.
    #[must_use]
    pub fn from_local_time(value: NaiveTime) -> Self {
        Self(value)
    }

    /// Returns local time without a date or time zone. Corresponds to Java: `toLocalTime`.
    #[must_use]
    pub fn to_local_time(self) -> NaiveTime {
        self.0
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.format("%H:%M:%S"))
    }
}

impl From<Time> for Value {
    fn from(value: Time) -> Self {
        Self::Time(value.0)
    }
}
