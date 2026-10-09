#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::statement::Statement + ?Sized>() {}
