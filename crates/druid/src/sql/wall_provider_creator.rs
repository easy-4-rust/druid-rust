//! 对应 Java：`com.alibaba.druid.wall.WallProviderCreator`。

use super::{DbType, WallConfig, WallProvider};

/// 自定义数据库 Wall Provider 的发现协议。
///
/// Java 使用 `ServiceLoader`。Rust 使用 `inventory` 注册构造器，但保留
/// datasource 名称、URL、可空配置、可空数据库类型和优先级这五个可观察输入。
pub trait WallProviderCreator: Send + Sync + 'static {
    /// 尝试为数据库创建 Provider；不支持时返回 `None`。
    fn create_wall_config(
        &self,
        data_source_name: Option<&str>,
        data_source_url: Option<&str>,
        config: Option<&WallConfig>,
        db_type: Option<DbType>,
    ) -> Option<WallProvider>;

    /// 返回选择顺序；小值优先。
    fn order(&self) -> i32 {
        0
    }
}

/// Rust `inventory` 中的 `WallProviderCreator` 注册项。
pub struct WallProviderCreatorRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Box<dyn WallProviderCreator>,
}

inventory::collect!(WallProviderCreatorRegistration);

/// 返回按 Java `getOrder`、再按稳定名称排序的 Creator。
///
/// Java 源码虽然构建了排序列表，却误迭代原 ServiceLoader；Rust 不继承
/// `ClassLoader` 的偶然顺序，使用接口公开的 `getOrder` 作为确定性合同。
#[must_use]
pub fn registered_wall_provider_creators() -> Vec<Box<dyn WallProviderCreator>> {
    let mut creators = inventory::iter::<WallProviderCreatorRegistration>
        .into_iter()
        .map(|registration| (registration.name, (registration.constructor)()))
        .collect::<Vec<_>>();
    creators.sort_by(|left, right| {
        left.1
            .order()
            .cmp(&right.1.order())
            .then_with(|| left.0.cmp(right.0))
    });
    creators.into_iter().map(|(_, creator)| creator).collect()
}
