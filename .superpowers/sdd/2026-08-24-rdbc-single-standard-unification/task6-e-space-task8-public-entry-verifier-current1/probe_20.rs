#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::result_set::ResultSet + ?Sized>() {}
