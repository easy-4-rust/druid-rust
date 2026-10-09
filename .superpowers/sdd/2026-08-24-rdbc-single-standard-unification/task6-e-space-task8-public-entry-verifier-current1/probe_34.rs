#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::resource::SqlOutput + ?Sized>() {}
