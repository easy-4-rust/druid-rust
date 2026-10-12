//! 产品注册表保持既有的 panic 边界自动 trait 合同。
#![cfg(feature = "driver-catalog")]

use druid_wrapper::driver::DruidDriverRegistry;
use std::panic::{RefUnwindSafe, UnwindSafe};

fn assert_unwind_safe<T: RefUnwindSafe + UnwindSafe>() {}

#[test]
fn product_registry_preserves_unwind_safety() {
    assert_unwind_safe::<DruidDriverRegistry>();
}
