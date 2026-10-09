use super::cursor_state::CursorState;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize};
use std::sync::{Arc, Mutex};

/// 记录真实池化代理对驱动 getter、结果和 Filter 的操作。
/// 对应 Java: java.sql.Statement 的驱动测试状态。
pub struct DriverState {
    pub first: Arc<CursorState>,
    pub third: Arc<CursorState>,
    pub current: AtomicI32,
    pub getter_calls: AtomicUsize,
    pub statement_closed: AtomicBool,
    pub connection_closed: AtomicBool,
    pub open_ids: Mutex<Vec<u64>>,
    pub read_ids: Mutex<Vec<u64>>,
}
impl DriverState {
    /// 无参数；返回固定 7、8 / 更新数2 / 20 的三段结果。
    pub fn new() -> Self {
        Self {
            first: Arc::new(CursorState::new(&[7, 8])),
            third: Arc::new(CursorState::new(&[20])),
            current: AtomicI32::new(-1),
            getter_calls: AtomicUsize::new(0),
            statement_closed: AtomicBool::new(false),
            connection_closed: AtomicBool::new(false),
            open_ids: Mutex::new(Vec::new()),
            read_ids: Mutex::new(Vec::new()),
        }
    }
}
