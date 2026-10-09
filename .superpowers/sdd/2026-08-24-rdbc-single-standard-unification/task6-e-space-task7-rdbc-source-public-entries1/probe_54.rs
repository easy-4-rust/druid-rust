#![forbid(unsafe_code)]
pub fn probe<T: druid_rdbc::data_source::ConnectionEventListener + ?Sized>() {}
