use crate::core::ProxyAttributes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32};

/// 同一逻辑结果集的共享游标、属性与列映射；取得当前结果不能重新打开游标。
/// 对应 Java: com.alibaba.druid.pool.DruidPooledResultSet。
pub(super) struct SharedResultSetState {
    pub(super) id: u64,
    pub(super) attributes: ProxyAttributes,
    pub(super) closed: AtomicBool,
    pub(super) cursor_index: AtomicI32,
    pub(super) logic_column_map: RwLock<Option<HashMap<i32, i32>>>,
    pub(super) driver_column_map: RwLock<Option<HashMap<i32, i32>>>,
    pub(super) hidden_columns: RwLock<Option<Vec<i32>>>,
}
