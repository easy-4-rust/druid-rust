//! Java `FilterAdapter` 默认适配语义与真实 Toasty SQLite 验证。

use druid::core::{
    AfterFilter, BeforeFilter, DruidError, DruidPooledConnection, ExtendedFilter, FilterAdapter,
    FilterChain, PhysicalConnectionFactory, Wrapper, WrapperExt,
};
use druid::toasty::ToastyConnectionFactory;
use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

#[tokio::test]
async fn filter_adapter_preserves_lifecycle_configuration_and_exact_wrapper_identity() {
    let mut adapter = FilterAdapter::new();

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL：
    // Java 源码显示 init/destroy/configFromProperties 默认不产生副作用或错误；
    // Java 仓没有对应可执行测试，所以不能标成 V2_MIRRORED。
    assert_eq!(BeforeFilter::name(&adapter), "FilterAdapter");
    assert_eq!(AfterFilter::name(&adapter), "FilterAdapter");
    BeforeFilter::init(&adapter).await.unwrap();
    let properties = HashMap::from([
        ("enabled".to_string(), "true".to_string()),
        ("threshold".to_string(), "17".to_string()),
    ]);
    ExtendedFilter::config_from_properties(&mut adapter, &properties)
        .await
        .unwrap();
    BeforeFilter::destroy(&adapter).await.unwrap();

    // SOURCE_PARITY / V0_STATIC + V1_RUST_LOCAL：
    // Java 源码只对运行时自身 class 返回 true/自身，对 null 或其他类型返回
    // false/null；这里是 Rust 本地契约证据，不是 Java/Rust differential。
    assert!(WrapperExt::is_wrapper_for_type::<FilterAdapter>(&adapter));
    assert!(!WrapperExt::is_wrapper_for_type::<String>(&adapter));
    assert!(!Wrapper::is_wrapper_for(&adapter, None));
    assert!(Wrapper::unwrap(&adapter, None).is_none());
    let unwrapped = WrapperExt::unwrap_ref::<FilterAdapter>(&adapter).unwrap();
    assert!(ptr::eq(unwrapped, &adapter));
    assert!(Wrapper::unwrap(&adapter, Some(TypeId::of::<String>())).is_none());

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // 既有 ExtendedFilter 字符串类型令牌也必须严格匹配 Rust 运行时类型。
    assert!(ExtendedFilter::is_wrapper_for(
        &adapter,
        type_name::<FilterAdapter>()
    ));
    assert!(!ExtendedFilter::is_wrapper_for(&adapter, "FilterAdapter"));
}

#[tokio::test]
async fn filter_adapter_passes_all_registered_views_through_real_toasty_sqlite() {
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(Arc::new(FilterAdapter::new()));

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // Java 一个 Filter 实例同时进入 SQL before/after 与 ResultSet 三个视图。
    assert_eq!(filter_chain.before_count(), 1);
    assert_eq!(filter_chain.after_count(), 1);
    assert_eq!(filter_chain.result_set_count(), 1);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 Toasty SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        99,
        "sqlite-filter-adapter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT '你好，FilterAdapter'")
        .await
        .unwrap();

    // VALUE_ADD / V5_HOST：
    // 默认 Adapter 必须继续真实 SQL、游标与 getter 调用，不能返回常量或吞掉调用。
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.n_string(&mut connection, 1).unwrap(),
        Some("你好，FilterAdapter".to_string())
    );

    // VALUE_ADD / V5_HOST：
    // 默认 ResultSet 委托也不能吞掉真实只读 RowSet 的 capability error。
    assert_eq!(
        result_set.update_n_string(&mut connection, 1, Some("更新".to_string())),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();
}
