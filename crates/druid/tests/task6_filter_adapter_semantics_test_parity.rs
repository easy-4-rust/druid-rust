//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Java `FilterAdapter` 默认适配语义与真实 Toasty `SQLite` 验证。

use druid::core::{
    AfterFilter, BeforeFilter, Connection, ExtendedFilter, FilterAdapter, FilterChain, RdbcError,
    Wrapper, WrapperExt,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::{type_name, TypeId};
use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use druid_rdbc::result_set::ResultSetExt;

#[tokio::test]
async fn task6_filter_adapter_semantics_test_filter_adapter_preserves_lifecycle_configuration_and_exact_wrapper_identity(
) {
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
    assert!(matches!(
        Wrapper::is_wrapper_for(&adapter, None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        Wrapper::unwrap(&adapter, None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert_eq!(
        Wrapper::is_wrapper_for(&adapter, Some(TypeId::of::<dyn Wrapper>())),
        Ok(true)
    );
    assert!(matches!(
        Wrapper::unwrap(&adapter, Some(TypeId::of::<dyn Wrapper>())),
        Ok(druid::core::StandardUnwrapped::Wrapper(_))
    ));
    let unwrapped = WrapperExt::unwrap_ref::<FilterAdapter>(&adapter).unwrap();
    assert!(ptr::eq(unwrapped, &raw const adapter));
    assert_eq!(
        Wrapper::is_wrapper_for(&adapter, Some(TypeId::of::<String>())),
        Ok(false)
    );
    let unknown = Wrapper::unwrap(&adapter, Some(TypeId::of::<String>()))
        .expect_err("unknown canonical target must be structured unsupported");
    assert_eq!(
        unknown.kind(),
        druid_rdbc::error::RdbcErrorKind::SqlFeatureNotSupported
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：
    // 既有 ExtendedFilter 字符串类型令牌也必须严格匹配 Rust 运行时类型。
    assert!(ExtendedFilter::is_wrapper_for(
        &adapter,
        type_name::<FilterAdapter>()
    ));
    assert!(!ExtendedFilter::is_wrapper_for(&adapter, "FilterAdapter"));
}

#[tokio::test]
async fn task6_filter_adapter_semantics_test_filter_adapter_passes_all_registered_views_through_real_toasty_sqlite(
) {
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
    let pool = druid::pool::DruidPool::builder()
        .name("sqlite-filter-adapter")
        .driver_name("toasty")
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(filter_chain))
        .build()
        .await
        .expect("canonical pool must build");
    let mut connection = pool.get().await.expect("canonical lease must be acquired");
    let mut statement = Connection::create_statement(
        &mut connection,
        druid_rdbc::statement::StatementOptions::default(),
    )
    .await
    .unwrap();
    let mut result_set = statement
        .execute_query("SELECT '你好，FilterAdapter'")
        .await
        .unwrap();

    // VALUE_ADD / V5_HOST：
    // 默认 Adapter 必须继续真实 SQL、游标与 getter 调用，不能返回常量或吞掉调用。
    assert!(result_set.next().await.unwrap());
    let column = druid_rdbc::result_set::ColumnIndex::new(1).unwrap();
    assert_eq!(
        result_set.n_string(column).unwrap(),
        Some("你好，FilterAdapter".to_string())
    );

    // VALUE_ADD / V5_HOST：
    // 默认 ResultSet 委托也不能吞掉真实只读 RowSet 的 capability error。
    assert_eq!(
        result_set.update_value(
            column,
            druid_rdbc::result_set::ResultSetUpdate::NString(Some("更新".to_string()),),
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    );
    assert_eq!(pool.stat_value_and_reset().error_count, 1);
    result_set.close().await.unwrap();
}
