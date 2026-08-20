//! AutoLoad 全量覆盖测试（Java `AutoLoad.java` 差分对照）。
//!
//! 覆盖目标：
//! - AutoLoad::new 构造、value/filter_class_name/order 访问器
//! - AutoLoad::with_order 构造、显式 order
//! - Default order = 0
//! - with_order 自定义 order

extern crate druid_core as druid;
use druid_core::core::{AutoLoad, FilterManager};

/// 无副作用的 register 空函数。
fn noop_register(_manager: &FilterManager) {}

/// AutoLoad::new 默认 order = 0。
#[test]
fn auto_load_new_default_order_is_zero() {
    let auto = AutoLoad::new("com.example.MyFilter", true, noop_register);
    assert_eq!(auto.order(), 0);
    assert_eq!(auto.filter_class_name(), "com.example.MyFilter");
    assert!(auto.value());
}

/// AutoLoad::new value=false。
#[test]
fn auto_load_new_value_false() {
    let auto = AutoLoad::new("com.example.DisabledFilter", false, noop_register);
    assert!(!auto.value());
    assert_eq!(auto.filter_class_name(), "com.example.DisabledFilter");
    assert_eq!(auto.order(), 0);
}

/// AutoLoad::with_order 自定义 order。
#[test]
fn auto_load_with_order_custom_order() {
    let auto = AutoLoad::with_order("com.example.OrderedFilter", true, 42, noop_register);
    assert_eq!(auto.order(), 42);
    assert_eq!(auto.filter_class_name(), "com.example.OrderedFilter");
    assert!(auto.value());
}

/// AutoLoad::with_order 负数 order。
#[test]
fn auto_load_with_order_negative_order() {
    let auto = AutoLoad::with_order("com.example.NegFilter", true, -10, noop_register);
    assert_eq!(auto.order(), -10);
    assert!(auto.value());
}

/// AutoLoad::with_order value=false + 显式 order。
#[test]
fn auto_load_with_order_disabled_with_order() {
    let auto = AutoLoad::with_order("com.example.DisabledOrdered", false, 100, noop_register);
    assert!(!auto.value());
    assert_eq!(auto.order(), 100);
    assert_eq!(auto.filter_class_name(), "com.example.DisabledOrdered");
}

/// filter_class_name 是 static 生命周期。
#[test]
fn auto_load_filter_class_name_is_static() {
    let class_name: &'static str = "com.example.StaticFilter";
    let auto = AutoLoad::new(class_name, true, noop_register);
    let returned: &'static str = auto.filter_class_name();
    assert_eq!(returned, class_name);
}

/// value getter 与构造参数一致。
#[test]
fn auto_load_value_getter_matches_constructor() {
    let auto_true = AutoLoad::new("a", true, noop_register);
    let auto_false = AutoLoad::new("b", false, noop_register);
    assert!(auto_true.value());
    assert!(!auto_false.value());
}

/// order getter 与构造参数一致。
#[test]
fn auto_load_order_getter_matches_constructor() {
    let default_order = AutoLoad::new("c", true, noop_register);
    let explicit_order = AutoLoad::with_order("d", true, 999, noop_register);
    assert_eq!(default_order.order(), 0);
    assert_eq!(explicit_order.order(), 999);
}

/// 空类名也是合法输入。
#[test]
fn auto_load_empty_class_name() {
    let auto = AutoLoad::new("", true, noop_register);
    assert_eq!(auto.filter_class_name(), "");
}

/// 极大 order 值。
#[test]
fn auto_load_max_order() {
    let auto = AutoLoad::with_order("max", true, i32::MAX, noop_register);
    assert_eq!(auto.order(), i32::MAX);
}

/// 极小 order 值。
#[test]
fn auto_load_min_order() {
    let auto = AutoLoad::with_order("min", true, i32::MIN, noop_register);
    assert_eq!(auto.order(), i32::MIN);
}

/// new 和 with_order 在 value=true, order=0 时语义等价。
#[test]
fn auto_load_new_and_with_order_equivalent_at_zero() {
    let a = AutoLoad::new("com.example.Equiv", true, noop_register);
    let b = AutoLoad::with_order("com.example.Equiv", true, 0, noop_register);
    assert_eq!(a.value(), b.value());
    assert_eq!(a.filter_class_name(), b.filter_class_name());
    assert_eq!(a.order(), b.order());
}
