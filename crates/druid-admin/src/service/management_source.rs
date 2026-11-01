//! 管理数据来源。
//!
//! Java 的 Admin 只认一种来源：发现节点后逐个拉取 `/druid/*.json`。
//! Rust 的 Admin 收 gRPC 推送，把最新值存在内存仓库里。两者的信息量
//! 差别很大：推送协议只带约十个数值字段，而活池里还有 URL、池配置、
//! 直方图、错误明细、池化连接与活跃连接堆栈。
//!
//! 本类型把两者统一成一个来源，并固定优先级：**远端优先，仓库兜底**。
//! 远端可达时用远端（信息更全）；远端不可达或没有节点时用仓库
//! （推送始终可用）。这样同一个控制台既能对接能提供 `/druid/*.json`
//! 的旧式应用，也能对接只提供 gRPC 推送的现代应用。

use std::sync::Arc;

use serde_json::{json, Value};

use druid_metrics::ServiceIdentity;

use crate::model::dto::{
    ConnectionResult, DataSourceContent, DataSourceResult, SqlListContent, WallResult,
};
use crate::repository::MetricsRepository;
use crate::service::{MonitorStatService, MonitorStatServiceError, StatQuery};

/// 管理数据来源。
#[derive(Clone)]
pub struct ManagementSource {
    remote: Option<Arc<MonitorStatService>>,
    repository: MetricsRepository,
}

impl ManagementSource {
    /// 使用仓库视图创建来源。
    #[must_use]
    pub fn repository(repository: MetricsRepository) -> Self {
        Self {
            remote: None,
            repository,
        }
    }

    /// 使用远端视图创建来源。
    #[must_use]
    pub fn remote(remote: Arc<MonitorStatService>) -> Self {
        Self {
            remote: Some(remote),
            repository: MetricsRepository::new(),
        }
    }

    /// 同时使用远端与仓库。
    #[must_use]
    pub fn both(remote: Arc<MonitorStatService>, repository: MetricsRepository) -> Self {
        Self {
            remote: Some(remote),
            repository,
        }
    }

    /// 返回是否配置了远端来源。
    #[must_use]
    pub fn has_remote(&self) -> bool {
        self.remote.is_some()
    }

    /// 返回推送仓库视图。
    #[must_use]
    pub fn repository_view(&self) -> &MetricsRepository {
        &self.repository
    }

    /// 返回远端来源；未配置时为 None。
    ///
    /// 只有远端独有的数据（Web/Spring 家族、池化连接、活跃连接堆栈）
    /// 需要直接访问它。
    #[must_use]
    pub fn remote_service(&self) -> Option<&Arc<MonitorStatService>> {
        self.remote.as_ref()
    }

    /// 返回数据源列表。
    ///
    /// 远端可达且返回了数据时用远端，否则用仓库。
    pub async fn data_sources(&self) -> Result<DataSourceResult, MonitorStatServiceError> {
        if let Some(remote) = &self.remote {
            if let Ok(result) = remote.get_data_source_stat_data().await {
                if result.content.as_ref().is_some_and(|rows| !rows.is_empty()) {
                    return Ok(result);
                }
            }
        }
        Ok(DataSourceResult {
            result_code: MonitorStatService::RESULT_CODE_SUCCESS,
            content: Some(self.repository_datasources()),
        })
    }

    /// 返回单个数据源。
    ///
    /// 身份是 `(service_identity, identity)` 的复合键：同一个 identity 可能属于
    /// 多个服务（注册中心存在名称冲突），不能只按 identity 匹配。
    pub async fn data_source(
        &self,
        identity: i64,
        service_id: Option<&str>,
    ) -> Result<Option<DataSourceContent>, MonitorStatServiceError> {
        if let Some(remote) = &self.remote {
            if let Ok(result) = remote.get_data_source_stat_data().await {
                if let Some(found) = result
                    .content
                    .unwrap_or_default()
                    .into_iter()
                    .find(|content| content.identity == identity)
                {
                    return Ok(Some(found));
                }
            }
        }
        // 仓库路径：给了 serviceId 就用复合键精确解析（同名 identity 分属不同服务）；
        // 没带 serviceId（Java 兼容路由）时按 identity 直查。
        if let Some(service_id) = service_id {
            let Ok(service_identity) = ServiceIdentity::parse(service_id) else {
                return Ok(None);
            };
            return Ok(self
                .repository_view()
                .get_datasource(&crate::repository::DataSourceId {
                    service_identity,
                    identity,
                })
                .map(|entry| entry.datasource));
        }
        Ok(self
            .repository_datasources()
            .into_iter()
            .find(|content| content.identity == identity))
    }

    /// 返回类型化的数据源行，供版本化 API 做字段映射。
    pub async fn datasource_rows(&self) -> Result<Vec<DataSourceContent>, MonitorStatServiceError> {
        Ok(self.data_sources().await?.content.unwrap_or_default())
    }

    /// 返回类型化的 SQL 统计行，供版本化 API 做字段映射。
    pub async fn sql_rows(
        &self,
        query: &StatQuery,
    ) -> Result<Vec<SqlListContent>, MonitorStatServiceError> {
        let value = self.sql_list(query).await?;
        Ok(value
            .get("Content")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| serde_json::from_value::<SqlListContent>(row.clone()).ok())
                    .collect()
            })
            .unwrap_or_default())
    }

    /// 返回 SQL 统计列表。
    pub async fn sql_list(&self, query: &StatQuery) -> Result<Value, MonitorStatServiceError> {
        if let Some(remote) = &self.remote {
            if let Ok(result) = remote.get_sql_stat_data_list(query).await {
                let rows = result
                    .get("Content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if !rows.is_empty() {
                    return Ok(result);
                }
            }
        }
        Ok(json!({
            "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
            "Content": self.repository_sql_stats(),
        }))
    }

    /// 返回单条 SQL 统计，Java 形状。
    ///
    /// 身份是 `(service_identity, datasource_identity, sql_identity)` 的复合键；
    /// 同一个 sql identity 可能属于多个服务或多个数据源。
    pub async fn sql_stat(
        &self,
        identity: i64,
        service_id: Option<&str>,
        datasource_id: Option<i64>,
    ) -> Result<Option<Value>, MonitorStatServiceError> {
        if let Some(remote) = &self.remote {
            if let Some(service_id) = service_id {
                if let Ok(result) = remote.get_sql_stat(identity, service_id).await {
                    if let Some(content) = result.content {
                        return Ok(Some(serde_json::to_value(content).unwrap_or(Value::Null)));
                    }
                }
            }
        }
        // 仓库路径：有复合键时用复合键精确解析，否则按 id/hash 全局查找。
        if let (Some(service_id), Some(datasource_id)) = (service_id, datasource_id) {
            if let Ok(service_identity) = ServiceIdentity::parse(service_id) {
                return Ok(self
                    .repository_view()
                    .get_sql_stat(&service_identity, datasource_id, identity)
                    .and_then(|stat| serde_json::to_value(stat).ok()));
            }
            return Ok(None);
        }
        Ok(self
            .repository_sql_stats()
            .into_iter()
            .find(|stat| stat.id == identity || stat.hash == identity)
            .and_then(|stat| serde_json::to_value(stat).ok()))
    }

    /// 返回 SQL 防火墙统计。
    pub async fn wall(&self) -> Result<WallResult, MonitorStatServiceError> {
        if let Some(remote) = &self.remote {
            if let Ok(result) = remote.get_wall_stat_map(&StatQuery::default()).await {
                if result.content.check_count > 0 || result.content.violation_count > 0 {
                    return Ok(result);
                }
            }
        }
        let mut wall = self.repository.aggregated_wall();
        wall.result_code = MonitorStatService::RESULT_CODE_SUCCESS;
        Ok(wall)
    }

    /// 返回池化连接明细。
    pub async fn connection_info(
        &self,
        identity: i64,
        service_id: Option<&str>,
    ) -> Result<Option<ConnectionResult>, MonitorStatServiceError> {
        let Some(remote) = &self.remote else {
            return Ok(None);
        };
        let Some(service_id) = service_id else {
            return Ok(None);
        };
        remote
            .get_pooling_connection_info_by_data_source_id(identity, service_id)
            .await
            .map(Some)
    }

    /// 返回已接入的服务名。
    pub fn services(&self) -> Vec<String> {
        let mut names: Vec<String> = match &self.remote {
            Some(remote) => remote.applications().to_vec(),
            None => Vec::new(),
        };
        for name in self
            .repository
            .all_datasources()
            .into_iter()
            .filter_map(|entry| entry.datasource.service_name)
        {
            names.push(name.to_string());
        }
        names.sort();
        names.dedup();
        names
    }

    /// 返回仓库中的数据源列表。
    fn repository_datasources(&self) -> Vec<DataSourceContent> {
        self.repository
            .all_datasources()
            .into_iter()
            .map(|entry| entry.datasource)
            .collect()
    }

    /// 返回仓库中的 SQL 统计列表。
    fn repository_sql_stats(&self) -> Vec<SqlListContent> {
        self.repository.all_sql_stats()
    }
}

impl std::fmt::Debug for ManagementSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementSource")
            .field("remote", &self.remote.is_some())
            .field("repository", &"[INTERNAL]")
            .finish()
    }
}
