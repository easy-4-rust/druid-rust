/// Standard reason why `Connection#setClientInfo` could not set a property.
///
/// Corresponds to Java: `java.sql.ClientInfoStatus`; reported per property by
/// `SQLClientInfoException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientInfoStatus {
    /// The reason is unknown.
    ReasonUnknown,
    /// The data source does not recognize the property name.
    ReasonUnknownProperty,
    /// The property value is invalid.
    ReasonValueInvalid,
    /// The property value was truncated while stored or transferred.
    ReasonValueTruncated,
}
