/// Standard mapping of an SQL `XML` value.
///
/// Corresponds to Java: `java.sql.SQLXML`. It supports strings, binary and character streams,
/// and XML source/result forms. Access after `free` and XML conversion failures return errors.
pub use crate::core::RdbcSqlXml as SqlXml;
