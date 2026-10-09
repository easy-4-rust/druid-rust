#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::extensions::RowSetWriter + ?Sized>() {}
