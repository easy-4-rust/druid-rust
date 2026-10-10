use super::row_set_cursor::RowSetCursor;
use super::{
    RowSetListener, RowSetParameterResolver, RowSetParameterStore, RowSetProperties, RowSetReader,
    RowSetWriter,
};
use std::sync::{Arc, Mutex};

/// 单一逻辑 RowSet 的共享 owner；锁内只执行短同步状态操作。
/// 对应 Java: javax.sql.RowSet 对象的属性、listener 与 provider 登记（Rust 内部扩展）。
pub(crate) struct RowSetState {
    pub(crate) closed: bool,
    pub(crate) busy: Option<u64>,
    pub(crate) next_operation: u64,
    pub(crate) properties: RowSetProperties,
    pub(crate) parameters: RowSetParameterStore,
    pub(crate) resolver: Option<RowSetParameterResolver>,
    pub(crate) reader: Option<Arc<tokio::sync::Mutex<Box<dyn RowSetReader>>>>,
    pub(crate) writer: Option<Arc<tokio::sync::Mutex<Box<dyn RowSetWriter>>>>,
    pub(crate) listeners: Vec<Arc<dyn RowSetListener>>,
    pub(crate) cursor: Arc<Mutex<RowSetCursor>>,
}
impl RowSetState {
    pub(crate) fn new() -> Self {
        Self {
            closed: false,
            busy: None,
            next_operation: 0,
            properties: RowSetProperties::new(),
            parameters: RowSetParameterStore::new(),
            resolver: None,
            reader: None,
            writer: None,
            listeners: Vec::new(),
            cursor: Arc::new(Mutex::new(RowSetCursor::new())),
        }
    }
}
