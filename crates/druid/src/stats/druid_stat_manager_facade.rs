use super::DruidDataSourceStatManager;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

/// Druid 管理统计统一门面。
///
/// 对应 Java：`com.alibaba.druid.stat.DruidStatManagerFacade`。
pub struct DruidStatManagerFacade {
    reset_enable: AtomicBool,
    reset_count: AtomicU64,
}

impl DruidStatManagerFacade {
    /// 返回进程级门面。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<DruidStatManagerFacade> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            reset_enable: AtomicBool::new(true),
            reset_count: AtomicU64::new(0),
        })
    }

    /// 返回是否允许 reset。
    #[must_use]
    pub fn is_reset_enable(&self) -> bool {
        self.reset_enable.load(Ordering::Acquire)
    }

    /// 设置 reset 开关。
    pub fn set_reset_enable(&self, reset_enable: bool) {
        self.reset_enable.store(reset_enable, Ordering::Release);
    }

    /// 重置全部统计。
    pub fn reset_all(&self) {
        if !self.is_reset_enable() {
            return;
        }
        DruidDataSourceStatManager::global().reset();
        self.reset_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回 reset 次数。
    #[must_use]
    pub fn reset_count(&self) -> u64 {
        self.reset_count.load(Ordering::Acquire)
    }

    /// 返回 basic.json 内容。
    #[must_use]
    pub fn basic_stat(&self) -> Value {
        json!({
            "Version": env!("CARGO_PKG_VERSION"),
            "Drivers": [],
            "ResetEnable": self.is_reset_enable(),
            "ResetCount": self.reset_count(),
            "JavaVersion": Value::Null,
            "RustVersion": option_env!("RUSTC_VERSION"),
        })
    }

    /// 返回 datasource.json 内容。
    #[must_use]
    pub fn data_source_stat_data_list(&self) -> Vec<Value> {
        DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .map(|(id, data_source)| {
                let mut value = data_source.data_source_stat_data();
                if let Some(map) = value.as_object_mut() {
                    map.insert("Identity".to_owned(), id.into());
                    map.entry("Name".to_owned())
                        .or_insert_with(|| data_source.name().into());
                }
                value
            })
            .collect()
    }

    /// 返回指定数据源内容。
    #[must_use]
    pub fn data_source_stat_data(&self, id: u64) -> Option<Value> {
        let data_source = DruidDataSourceStatManager::global().get(id)?;
        let mut value = data_source.data_source_stat_data();
        if let Some(map) = value.as_object_mut() {
            map.insert("Identity".to_owned(), id.into());
        }
        Some(value)
    }

    /// 返回 SQL 统计列表，可按数据源 ID 筛选。
    #[must_use]
    pub fn sql_stat_data_list(&self, data_source_id: Option<u64>) -> Vec<Value> {
        DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .filter(|(id, _)| data_source_id.is_none_or(|expected| expected == *id))
            .flat_map(|(id, data_source)| {
                data_source
                    .sql_stat_data()
                    .into_iter()
                    .map(move |mut value| {
                        if let Some(map) = value.as_object_mut() {
                            map.insert("DataSource".to_owned(), id.into());
                        }
                        value
                    })
            })
            .collect()
    }

    /// 返回指定 SQL ID 的统计。
    #[must_use]
    pub fn sql_stat_data(&self, sql_id: u64) -> Option<Value> {
        self.sql_stat_data_list(None).into_iter().find(|value| {
            value
                .get("ID")
                .and_then(Value::as_u64)
                .is_some_and(|id| id == sql_id)
        })
    }

    /// 返回指定数据源的空闲连接信息。
    #[must_use]
    pub fn pooling_connection_info(&self, data_source_id: u64) -> Option<Vec<Value>> {
        DruidDataSourceStatManager::global()
            .get(data_source_id)
            .map(|data_source| data_source.pooling_connection_info())
    }

    /// 返回所有数据源非空的活跃连接调用栈分组。
    #[must_use]
    pub fn active_connection_stack_trace_list(&self) -> Vec<Value> {
        DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .filter_map(|(_, data_source)| {
                let traces = data_source.active_connection_stack_trace();
                (!traces.is_empty()).then(|| json!(traces))
            })
            .collect()
    }

    /// 返回指定数据源的活跃连接调用栈。
    #[must_use]
    pub fn active_connection_stack_trace(&self, data_source_id: u64) -> Option<Vec<String>> {
        DruidDataSourceStatManager::global()
            .get(data_source_id)
            .map(|data_source| data_source.active_connection_stack_trace())
    }

    /// 返回合并 Wall 管理结果。
    #[must_use]
    pub fn wall_stat_data(&self, data_source_id: Option<u64>) -> Value {
        let values = DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .filter(|(id, _)| data_source_id.is_none_or(|expected| expected == *id))
            .map(|(_, data_source)| data_source.wall_stat_data())
            .collect::<Vec<_>>();
        let mut merged = serde_json::Map::new();
        for field in [
            "checkCount",
            "hardCheckCount",
            "violationCount",
            "violationEffectRowCount",
            "blackListHitCount",
            "blackListSize",
            "whiteListHitCount",
            "whiteListSize",
            "syntaxErrorCount",
        ] {
            let sum = values
                .iter()
                .filter_map(|value| value.get(field).and_then(Value::as_u64))
                .sum::<u64>();
            merged.insert(field.to_owned(), sum.into());
        }
        for field in ["tables", "functions", "blackList", "whiteList"] {
            let entries = values
                .iter()
                .filter_map(|value| value.get(field).and_then(Value::as_array))
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            merged.insert(field.to_owned(), entries.into());
        }
        Value::Object(merged)
    }
}
