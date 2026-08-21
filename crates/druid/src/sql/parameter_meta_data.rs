use std::any::Any;

use super::{RdbcType, Wrapper};

/// Mode of a `PreparedStatement` parameter.
///
/// Corresponds to Java `ParameterMetaData.parameterMode*` constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ParameterMode {
    /// The driver cannot determine the mode.
    Unknown = 0,
    /// Input parameter.
    In = 1,
    /// Combined input and output parameter.
    InOut = 2,
    /// Output parameter.
    Out = 4,
}

/// Nullability of a `PreparedStatement` parameter.
///
/// Corresponds to Java `ParameterMetaData` nullability constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ParameterNullability {
    /// SQL `NULL` is not allowed.
    NoNulls = 0,
    /// SQL `NULL` is allowed.
    Nullable = 1,
    /// The driver cannot determine nullability.
    Unknown = 2,
}

/// Describes the type and properties of each `PreparedStatement` parameter marker.
///
/// Corresponds to Java: `java.sql.ParameterMetaData`. Positions are 1-based. This Rust value
/// uses `None` for an invalid position; a Java-style boundary maps it to `SQLException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterMetaData {
    parameter_types: Vec<RdbcType>,
}

impl ParameterMetaData {
    /// Creates metadata from RDBC types in SQL declaration order.
    ///
    /// `parameter_types[0]` represents RDBC parameter position 1.
    #[must_use]
    pub fn new(parameter_types: Vec<RdbcType>) -> Self {
        Self { parameter_types }
    }

    /// Returns the number of parameter markers. Corresponds to Java: `getParameterCount`.
    #[must_use]
    pub fn parameter_count(&self) -> usize {
        self.parameter_types.len()
    }
    /// `snake_case` form of Java `getParameterCount`.
    #[must_use]
    pub fn get_parameter_count(&self) -> usize {
        self.parameter_count()
    }

    /// Returns the RDBC type at 1-based `parameter`, or `None` when out of range.
    #[must_use]
    pub fn parameter_type(&self, parameter: usize) -> Option<RdbcType> {
        parameter
            .checked_sub(1)
            .and_then(|index| self.parameter_types.get(index))
            .copied()
    }
    /// Returns the `java.sql.Types` number for a 1-based parameter.
    #[must_use]
    pub fn get_parameter_type(&self, parameter: usize) -> Option<i32> {
        self.parameter_type(parameter)
            .map(|value| super::rdbc_sql_type::SqlType::vendor_type_number(&value))
    }

    /// Returns the database type name for a 1-based parameter.
    #[must_use]
    pub fn parameter_type_name(&self, parameter: usize) -> Option<&'static str> {
        self.parameter_type(parameter)
            .map(|value| super::rdbc_sql_type::SqlType::name(&value))
    }
    /// `snake_case` form of Java `getParameterTypeName`.
    #[must_use]
    pub fn get_parameter_type_name(&self, parameter: usize) -> Option<&'static str> {
        self.parameter_type_name(parameter)
    }

    /// Returns the standard Rust mapped type name for the parameter value.
    ///
    /// Adapts Java `getParameterClassName` by returning a Rust type path.
    #[must_use]
    pub fn parameter_class_name(&self, parameter: usize) -> Option<&'static str> {
        self.parameter_type(parameter).map(|value| match value {
            RdbcType::Boolean | RdbcType::Bit => "bool",
            RdbcType::TinyInt | RdbcType::SmallInt | RdbcType::Integer | RdbcType::BigInt => "i64",
            RdbcType::Float | RdbcType::Real | RdbcType::Double => "f64",
            RdbcType::Numeric | RdbcType::Decimal => "bigdecimal::BigDecimal",
            RdbcType::Binary | RdbcType::VarBinary | RdbcType::LongVarBinary | RdbcType::Blob => {
                "Vec<u8>"
            }
            RdbcType::Date => "druid::sql::Date",
            RdbcType::Time | RdbcType::TimeWithTimezone => "druid::sql::Time",
            RdbcType::Timestamp | RdbcType::TimestampWithTimezone => "druid::sql::Timestamp",
            _ => "String",
        })
    }
    /// `snake_case` form of Java `getParameterClassName`.
    #[must_use]
    pub fn get_parameter_class_name(&self, parameter: usize) -> Option<&'static str> {
        self.parameter_class_name(parameter)
    }

    /// Returns the mode; a normal `PreparedStatement` parameter defaults to `In`.
    #[must_use]
    pub fn parameter_mode(&self, parameter: usize) -> Option<ParameterMode> {
        self.parameter_type(parameter).map(|_| ParameterMode::In)
    }
    /// `snake_case` form of Java `getParameterMode`.
    #[must_use]
    pub fn get_parameter_mode(&self, parameter: usize) -> Option<ParameterMode> {
        self.parameter_mode(parameter)
    }

    /// Returns nullability; `Unknown` means the driver supplied no detail.
    #[must_use]
    pub fn nullable(&self, parameter: usize) -> Option<ParameterNullability> {
        self.parameter_type(parameter)
            .map(|_| ParameterNullability::Unknown)
    }

    /// Compatibility getter for existing Rust callers.
    #[must_use]
    pub fn get_nullable(&self, parameter: usize) -> Option<ParameterNullability> {
        self.nullable(parameter)
    }

    /// `snake_case` form of Java `isNullable(int)`.
    #[must_use]
    pub fn is_nullable(&self, parameter: usize) -> Option<ParameterNullability> {
        self.nullable(parameter)
    }

    /// Returns whether the parameter is a signed numeric type.
    #[must_use]
    pub fn is_signed(&self, parameter: usize) -> Option<bool> {
        self.parameter_type(parameter).map(|value| {
            matches!(
                value,
                RdbcType::TinyInt
                    | RdbcType::SmallInt
                    | RdbcType::Integer
                    | RdbcType::BigInt
                    | RdbcType::Float
                    | RdbcType::Real
                    | RdbcType::Double
                    | RdbcType::Numeric
                    | RdbcType::Decimal
            )
        })
    }

    /// Returns precision; zero means unknown under RDBC metadata conventions.
    #[must_use]
    pub fn precision(&self, parameter: usize) -> Option<u32> {
        self.parameter_type(parameter).map(|_| 0)
    }

    /// `snake_case` form of Java `getPrecision`.
    #[must_use]
    pub fn get_precision(&self, parameter: usize) -> Option<u32> {
        self.precision(parameter)
    }

    /// Returns scale; zero means unknown when the driver supplies no detail.
    #[must_use]
    pub fn scale(&self, parameter: usize) -> Option<u32> {
        self.parameter_type(parameter).map(|_| 0)
    }

    /// `snake_case` form of Java `getScale`.
    #[must_use]
    pub fn get_scale(&self, parameter: usize) -> Option<u32> {
        self.scale(parameter)
    }
}

impl Wrapper for ParameterMetaData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
