use std::sync::atomic::{AtomicU8, Ordering};

/// 共享标准值释放调用的取消保护（Rust平台扩展）；对应 Java: java.sql.Array#free等的结果确认。
/// 未观察到驱动完成时保留待重试状态，避免永久Releasing或把未知结果误报为成功。
pub(crate) struct StandardResourceReleaseGuard<'a> {
    state: &'a AtomicU8,
    completed: bool,
}
impl<'a> StandardResourceReleaseGuard<'a> {
    pub(crate) fn new(state: &'a AtomicU8) -> Self {
        Self {
            state,
            completed: false,
        }
    }
    pub(crate) fn complete(&mut self) {
        self.completed = true;
    }
}
impl Drop for StandardResourceReleaseGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.state.store(3, Ordering::Release);
        }
    }
}
