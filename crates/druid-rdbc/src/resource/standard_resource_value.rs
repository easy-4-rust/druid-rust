use super::standard_resource_release_guard::StandardResourceReleaseGuard;
use crate::error::RdbcError;
use crate::resource::{ResourceIdentity, ResourceOperations};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};

/// 标准资源的异步释放回调；保留驱动借用生命周期及Send约束。
type StandardResourceRelease<T> =
    for<'a> fn(&'a mut T) -> Pin<Box<dyn Future<Output = Result<(), RdbcError>> + Send + 'a>>;

/// 唯一标准资源对象的共享值容器（Rust平台扩展）；对应 Java: java.sql.Array/Blob/Clob/SQLXML的引用身份。
/// 只串行调用Send驱动，不定义第二个端口、不持有Druid owner、不调度新的runtime任务。
pub(crate) struct StandardResourceValue<T: ?Sized + Send + 'static> {
    payload: tokio::sync::Mutex<Box<T>>,
    identity: ResourceIdentity,
    capabilities: ResourceOperations,
    name: &'static str,
    // 0=可用，1=释放调用中，2=已确认成功释放，3=释放调用取消或panic后待重试确认。
    state: AtomicU8,
}

impl<T: ?Sized + Send + 'static> StandardResourceValue<T> {
    pub(crate) fn new(
        payload: Box<T>,
        capabilities: ResourceOperations,
        name: &'static str,
    ) -> Self {
        Self {
            payload: tokio::sync::Mutex::new(payload),
            identity: ResourceIdentity::local(),
            capabilities,
            name,
            state: AtomicU8::new(0),
        }
    }
    pub(crate) fn resource_id(&self) -> &str {
        self.identity.as_str()
    }
    pub(crate) fn capabilities(&self) -> ResourceOperations {
        self.capabilities
    }
    pub(crate) fn is_freed(&self) -> bool {
        self.state.load(Ordering::Acquire) == 2
    }
    /// 检查参数capability/operation的目标准入条件，返回成功或原生命周期/能力错误。
    pub(crate) fn require(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        if self.state.load(Ordering::Acquire) != 0 {
            return Err(RdbcError::rdbc_resource_closed(self.name));
        }
        self.require_capability(capability, operation)
    }
    fn require_capability(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<(), RdbcError> {
        if self.capabilities.contains(capability) {
            Ok(())
        } else {
            Err(RdbcError::feature_not_supported(operation))
        }
    }
    pub(crate) async fn lock(
        &self,
        capability: ResourceOperations,
        operation: &'static str,
    ) -> Result<tokio::sync::MutexGuard<'_, Box<T>>, RdbcError> {
        self.require(capability, operation)?;
        let payload = self.payload.lock().await;
        // 等待锁期间可能已经释放，必须在调用驱动前再次检查关闭与能力优先级。
        self.require(capability, operation)?;
        Ok(payload)
    }
    pub(crate) async fn free(
        &self,
        operation: &'static str,
        release: StandardResourceRelease<T>,
    ) -> Result<(), RdbcError> {
        if self.is_freed() {
            return Ok(());
        }
        self.require_capability(ResourceOperations::FREE, operation)?;
        let mut payload = self.payload.lock().await;
        if self.is_freed() {
            return Ok(());
        }
        let previous_state = self.state.load(Ordering::Acquire);
        self.state.store(1, Ordering::Release);
        // 取消时不声称已释放，也不恢复可读；原驱动/池化资源的完成责任保持不变。
        let mut guard = StandardResourceReleaseGuard::new(&self.state);
        let result = release(payload.as_mut()).await;
        // 曾取消的释放结果仍未确认；重试失败不能重新开放可能已释放的资源。
        let next_state = if result.is_ok() {
            2
        } else if previous_state == 3 {
            3
        } else {
            0
        };
        self.state.store(next_state, Ordering::Release);
        guard.complete();
        result
    }
}
impl<T: ?Sized + Send + 'static> fmt::Debug for StandardResourceValue<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandardResourceValue")
            .field("identity", &self.identity)
            .field("capabilities", &self.capabilities)
            // 保留历史可用/已释放状态的Debug名称，同时准确区分未确认的释放。
            .field(
                "state",
                &format_args!(
                    "{}",
                    match self.state.load(Ordering::Acquire) {
                        0 => "Open",
                        1 => "Releasing",
                        2 => "Freed",
                        _ => "ReleaseUnconfirmed",
                    }
                ),
            )
            .finish_non_exhaustive()
    }
}
