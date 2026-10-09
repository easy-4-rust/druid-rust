//! 验证 ClientInfoStatus 的唯一标准身份及历史公开路径兼容性。

use druid::sql::client_info_status::ClientInfoStatus as ModuleStatus;
use druid::sql::ClientInfoStatus as ProductStatus;
use druid_rdbc::connection::ClientInfoStatus;
use std::collections::HashMap;

fn canonical_status(status: ProductStatus) -> ClientInfoStatus {
    status
}

fn historical_module_status(status: ClientInfoStatus) -> ModuleStatus {
    status
}

#[test]
fn property_failures_preserve_canonical_status_across_product_paths() {
    let product_failures = HashMap::from([
        ("unknown", ProductStatus::ReasonUnknown),
        ("property", ProductStatus::ReasonUnknownProperty),
        ("invalid", ProductStatus::ReasonValueInvalid),
        ("truncated", ProductStatus::ReasonValueTruncated),
    ]);
    // 直接赋值验证同一类型，而不是靠 Debug 文本或值转换伪造兼容性。
    let canonical_failures: HashMap<&str, ClientInfoStatus> = product_failures;
    assert_eq!(canonical_failures.len(), 4);
    assert_eq!(
        canonical_failures["property"],
        ClientInfoStatus::ReasonUnknownProperty
    );
    assert_eq!(
        canonical_failures["invalid"],
        ClientInfoStatus::ReasonValueInvalid
    );
    assert_eq!(
        canonical_failures["truncated"],
        ClientInfoStatus::ReasonValueTruncated
    );
    assert_eq!(
        canonical_failures["unknown"],
        ClientInfoStatus::ReasonUnknown
    );
}

#[test]
fn legacy_module_and_facade_return_the_canonical_type_without_conversion() {
    let canonical = canonical_status(ProductStatus::ReasonValueInvalid);
    let legacy = historical_module_status(canonical);
    assert_eq!(legacy, ModuleStatus::ReasonValueInvalid);
    assert_eq!(
        std::any::TypeId::of::<ClientInfoStatus>(),
        std::any::TypeId::of::<ModuleStatus>()
    );
}
