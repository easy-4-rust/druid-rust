use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::types::{Row, Value};
use std::sync::atomic::AtomicUsize;
use tokio::sync::Mutex;

/// getter 与 execute 返回的驱动句柄共用同一游标。
/// 对应 Java: java.sql.ResultSet 的驱动测试资源。
pub struct CursorState {
    pub rows: Mutex<BufferedRowSet>,
    pub getter_reads: AtomicUsize,
    pub original_reads: AtomicUsize,
    pub close_calls: AtomicUsize,
    pub getter_drops: AtomicUsize,
    pub original_drops: AtomicUsize,
    pub row_count: u64,
}
impl CursorState {
    /// 参数为固定测试行；返回共享游标状态。
    pub fn new(values: &[i64]) -> Self {
        Self {
            rows: Mutex::new(BufferedRowSet::with_column_labels(
                values
                    .iter()
                    .map(|value| Row::new(vec![Value::Int(*value)]))
                    .collect(),
                vec!["value".to_owned()],
            )),
            getter_reads: AtomicUsize::new(0),
            original_reads: AtomicUsize::new(0),
            close_calls: AtomicUsize::new(0),
            getter_drops: AtomicUsize::new(0),
            original_drops: AtomicUsize::new(0),
            row_count: values.len() as u64,
        }
    }
}
