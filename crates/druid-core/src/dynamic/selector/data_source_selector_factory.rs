//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.DataSourceSelectorFactory`。

use super::{
    DataSourceSelector, DataSourceSelectorEnum, NamedDataSourceSelector, RandomDataSourceSelector,
    StickyRandomDataSourceSelector,
};
use crate::dynamic::HighAvailableDataSource;
use std::sync::Arc;

/// Druid 内置选择器工厂。
#[derive(Debug, Default, Clone, Copy)]
pub struct DataSourceSelectorFactory;

impl DataSourceSelectorFactory {
    /// 按名称创建绑定到指定 HA 数据源的新选择器。
    #[must_use]
    pub fn get_selector(
        name: &str,
        data_source: &HighAvailableDataSource,
    ) -> Option<Arc<dyn DataSourceSelector>> {
        match DataSourceSelectorEnum::of(name)? {
            DataSourceSelectorEnum::ByName => {
                Some(Arc::new(NamedDataSourceSelector::new(data_source)))
            }
            DataSourceSelectorEnum::Random => {
                Some(Arc::new(RandomDataSourceSelector::new(data_source)))
            }
            DataSourceSelectorEnum::StickyRandom => {
                Some(Arc::new(StickyRandomDataSourceSelector::new(data_source)))
            }
        }
    }
}
