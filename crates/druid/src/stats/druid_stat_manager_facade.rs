use super::{DataSourceMonitorable, DruidDataSourceStatManager, JdbcStatManager};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Druid 管理统计统一门面。
///
/// 对应 Java：`com.alibaba.druid.stat.DruidStatManagerFacade`。
pub struct DruidStatManagerFacade {
    reset_enable: AtomicBool,
    reset_count: AtomicU64,
    start_time_millis: u64,
}

impl DruidStatManagerFacade {
    /// 返回进程级门面。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<DruidStatManagerFacade> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            reset_enable: AtomicBool::new(true),
            reset_count: AtomicU64::new(0),
            start_time_millis: epoch_millis(),
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
        // Java 顺序：Spring/Web（Rust 平台不适用）→ JdbcStatManager →
        // DruidDataSourceStatManager → facade resetCount。
        self.reset_sql_stat();
        self.reset_data_source_stat();
        self.reset_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 重置所有 Druid 数据源池统计。
    pub fn reset_data_source_stat(&self) {
        DruidDataSourceStatManager::global().reset();
    }

    /// 重置 JDBC 代理层及每个数据源的 JDBC 统计。
    pub fn reset_sql_stat(&self) {
        JdbcStatManager::global().reset();
    }

    /// 发布并重置全部数据源区间统计。
    pub fn log_and_reset_data_source(&self) {
        if !self.is_reset_enable() {
            return;
        }
        DruidDataSourceStatManager::global().log_and_reset_data_source();
    }

    /// 返回 reset 次数。
    #[must_use]
    pub fn reset_count(&self) -> u64 {
        self.reset_count.load(Ordering::Acquire)
    }

    /// 返回 basic.json 内容。
    #[must_use]
    pub fn basic_stat(&self) -> Value {
        let drivers = DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .filter_map(|(_, data_source)| data_source.driver_name().map(str::to_owned))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        json!({
            "Version": env!("CARGO_PKG_VERSION"),
            "Drivers": drivers,
            "ResetEnable": self.is_reset_enable(),
            "ResetCount": self.reset_count(),
            // 旧管理前端依赖这些键，但 Rust 进程不存在 JVM；保留 null，
            // 不能用 Rust 信息冒充 Java 运行时。
            "JavaVMName": Value::Null,
            "JavaVersion": Value::Null,
            "JavaClassPath": Value::Null,
            "StartTime": self.start_time_millis,
            "RustMSRV": env!("CARGO_PKG_RUST_VERSION"),
            "RustTargetOS": std::env::consts::OS,
            "RustTargetArch": std::env::consts::ARCH,
        })
    }

    /// 按名称返回首个已注册数据源。
    #[must_use]
    pub fn data_source_by_name(&self, name: &str) -> Option<Arc<dyn DataSourceMonitorable>> {
        DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .map(|(_, data_source)| data_source)
            .find(|data_source| data_source.name() == name)
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
            .flat_map(|(_, data_source)| data_source.sql_stat_data())
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
        let data_source = DruidDataSourceStatManager::global().get(data_source_id)?;
        data_source
            .is_remove_abandoned()
            .then(|| data_source.active_connection_stack_trace())
    }

    /// 返回合并 Wall 管理结果。
    #[must_use]
    pub fn wall_stat_data(&self, data_source_id: Option<u64>) -> Value {
        DruidDataSourceStatManager::global()
            .instances()
            .into_iter()
            .filter(|(id, _)| data_source_id.is_none_or(|expected| expected == *id))
            .map(|(_, data_source)| data_source.wall_stat_data())
            .fold(Value::Object(serde_json::Map::new()), |merged, value| {
                merge_wall_stat(&merged, &value)
            })
    }
}

/// 递归合并两个 Wall 管理快照。
///
/// 对应 Java：`DruidStatManagerFacade#mergeWallStat(Map, Map)`。
fn merge_wall_stat(left: &Value, right: &Value) -> Value {
    let Some(right) = right.as_object() else {
        return right.clone();
    };
    let Some(left) = left.as_object() else {
        return Value::Object(right.clone());
    };
    if left.is_empty() {
        return Value::Object(right.clone());
    }
    if right.is_empty() {
        return Value::Object(left.clone());
    }

    let mut merged = serde_json::Map::new();
    // Java 历史实现只遍历 mapB 的键；两侧 WallProvider map 键集合正常一致。
    for (key, right_value) in right {
        let left_value = left.get(key);
        let value = match left_value {
            None => right_value.clone(),
            Some(left_value) if left_value.is_null() => right_value.clone(),
            Some(left_value) if right_value.is_null() => left_value.clone(),
            Some(left_value) if key == "blackList" => merge_black_list(left_value, right_value),
            Some(left_value) => merge_wall_value(left_value, right_value),
        };
        merged.insert(key.clone(), value);
    }
    Value::Object(merged)
}

fn merge_wall_value(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Object(_), Value::Object(_)) => merge_wall_stat(left, right),
        (Value::Array(left), Value::Array(right))
            if left.iter().all(Value::is_number) && right.iter().all(Value::is_number) =>
        {
            // Java 对 long[] 的历史实现误加了数组长度而不是元素值；迁移保留
            // 该可观察行为，不能“顺手修正”。
            let length = left.len().max(right.len());
            (0..length)
                .map(|index| {
                    let mut value = 0_u64;
                    if index < left.len() {
                        value = value.wrapping_add(left.len() as u64);
                    }
                    if index < right.len() {
                        value = value.wrapping_add(right.len() as u64);
                    }
                    Value::from(value)
                })
                .collect::<Vec<_>>()
                .into()
        }
        (Value::Array(left), Value::Array(right)) => merge_named_list(left, right),
        (Value::String(_), Value::String(_)) => left.clone(),
        (Value::Number(left), Value::Number(right)) => Value::from(
            left.as_u64()
                .unwrap_or_default()
                .wrapping_add(right.as_u64().unwrap_or_default()),
        ),
        _ => right.clone(),
    }
}

fn merge_black_list(left: &Value, right: &Value) -> Value {
    let mut entries = IndexMap::<String, Value>::new();
    for value in left
        .as_array()
        .into_iter()
        .flatten()
        .chain(right.as_array().into_iter().flatten())
    {
        if entries.len() >= 1_000 {
            break;
        }
        let Some(sql) = value.get("sql").and_then(Value::as_str) else {
            continue;
        };
        let merged = entries
            .get(sql)
            .map_or_else(|| value.clone(), |old| merge_wall_stat(old, value));
        entries.insert(sql.to_owned(), merged);
    }
    entries.into_values().collect::<Vec<_>>().into()
}

fn merge_named_list(left: &[Value], right: &[Value]) -> Value {
    let mut mapped = std::collections::HashMap::<Option<String>, &Value>::new();
    for value in left {
        mapped.insert(
            value.get("name").and_then(Value::as_str).map(str::to_owned),
            value,
        );
    }
    right
        .iter()
        .map(|right_value| {
            let name = right_value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned);
            mapped.get(&name).map_or_else(
                || right_value.clone(),
                |left_value| merge_wall_stat(left_value, right_value),
            )
        })
        .collect::<Vec<_>>()
        .into()
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
