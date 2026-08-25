#![allow(dead_code)]

use druid_rdbc::wrapper::{Wrapper, WrapperExt};

struct Concrete;

druid_rdbc::impl_standard_wrapper!(Concrete => Wrapper);

fn forbidden(value: &dyn Wrapper) {
    let _ = value.unwrap_ref::<Concrete>();
}

fn main() {}
