//! 对应 Java 枚举：`com.alibaba.druid.pool.ha.selector.DataSourceSelectorEnum`。

/// Druid 内置 HA 选择器类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataSourceSelectorEnum {
    ByName,
    Random,
    StickyRandom,
}

impl DataSourceSelectorEnum {
    /// 返回 Java 配置使用的精确名称。
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ByName => "byName",
            Self::Random => "random",
            Self::StickyRandom => "stickyRandom",
        }
    }

    /// 按 Java Factory 的忽略大小写规则解析名称。
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        [Self::ByName, Self::Random, Self::StickyRandom]
            .into_iter()
            .find(|selector| selector.name().eq_ignore_ascii_case(name))
    }
}
