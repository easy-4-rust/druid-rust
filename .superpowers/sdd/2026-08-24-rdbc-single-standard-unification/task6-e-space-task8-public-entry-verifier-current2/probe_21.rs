#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::metadata::ResultSetMetaData + ?Sized>() {}
