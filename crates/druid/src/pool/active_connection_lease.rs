use std::sync::atomic::AtomicBool;
use std::sync::Weak;
use std::time::Instant;

/// remove-abandoned 扫描所需的安全租约快照。
///
/// Rust-only 内部对象。它只持有弱原子令牌，不持有或别名可变物理连接；扫描器
/// 可使过期租约失效，但物理关闭仍由租约所有者的下一次操作/Drop 完成。
pub(crate) struct ActiveConnectionLease {
    pub(crate) borrowed_at: Instant,
    pub(crate) lease_active: Weak<AtomicBool>,
    pub(crate) execution_running: Weak<AtomicBool>,
    pub(crate) connect_stack_trace: String,
}

impl ActiveConnectionLease {
    pub(crate) fn new(lease_active: Weak<AtomicBool>, execution_running: Weak<AtomicBool>) -> Self {
        Self {
            borrowed_at: Instant::now(),
            lease_active,
            execution_running,
            // Java 在 removeAbandoned 开启时记录借出线程堆栈。Rust 的
            // Backtrace 同样在借出点捕获，管理端读取时不再伪造空结果。
            connect_stack_trace: std::backtrace::Backtrace::force_capture().to_string(),
        }
    }
}
