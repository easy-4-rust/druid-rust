#![allow(dead_code)]

use druid_rdbc::connection::Connection;

struct WrongVariant;

druid_rdbc::impl_standard_wrapper!(WrongVariant => Connection);

fn main() {}
