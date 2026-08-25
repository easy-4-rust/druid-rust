use std::any::{Any, TypeId};
use std::sync::Arc;

use async_trait::async_trait;
use druid::core::{DruidPooledConnection, Pool};
use druid::dynamic::{DataSourceGroup, DynamicDataSource, SqlHint};
use druid::sql::{CommonDataSource, DataSource};
use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::Wrapper;

/// wrapper 层动态 RDBC 数据源，支持主从路由与数据源组热切换。
///
/// 对应 Druid `HighAvailableDataSource` 与 `javax.sql.DataSource` 的组合职责。
pub struct DynamicRdbcDataSource {
    inner: DynamicDataSource,
}

impl DynamicRdbcDataSource {
    /// 使用初始主从组创建动态数据源。
    #[must_use]
    pub fn new(initial: DataSourceGroup) -> Self {
        Self {
            inner: DynamicDataSource::new(initial),
        }
    }

    /// 根据读写意图选择 Druid pool。
    #[must_use]
    pub fn route(&self, hint: SqlHint) -> Arc<dyn Pool> {
        self.inner.route(hint)
    }

    /// 根据读写意图取得连接。
    pub async fn get_connection_for(
        &self,
        hint: SqlHint,
    ) -> Result<DruidPooledConnection, RdbcError> {
        self.inner
            .get_connection_for(hint)
            .await
            .map_err(RdbcError::from)
    }

    /// 原子切换整个数据源组。
    pub fn switch(&self, new_group: DataSourceGroup) {
        self.inner.switch(new_group);
    }

    /// 返回当前数据源组名称。
    #[must_use]
    pub fn current_name(&self) -> String {
        self.inner.current_name()
    }
}

impl Wrapper for DynamicRdbcDataSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl druid_rdbc::wrapper::Wrapper for DynamicRdbcDataSource {
    fn wrapper_view(&self) -> &dyn druid_rdbc::wrapper::Wrapper {
        self
    }

    fn canonical_unwrap(
        &self,
        target: TypeId,
    ) -> Option<druid_rdbc::wrapper::StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn druid_rdbc::wrapper::Wrapper>())
            .then_some(druid_rdbc::wrapper::StandardUnwrapped::Wrapper(self))
    }
}

impl CommonDataSource for DynamicRdbcDataSource {}

#[async_trait]
impl DataSource for DynamicRdbcDataSource {
    async fn get_connection(&self) -> Result<DruidPooledConnection, RdbcError> {
        self.inner.get_connection().await.map_err(RdbcError::from)
    }
}
