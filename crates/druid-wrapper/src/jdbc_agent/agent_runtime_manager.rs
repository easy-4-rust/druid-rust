use super::{AgentRuntime, JdbcAgentOptions};
use druid::core::DruidError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;

/// 按 Agent 命令身份共享子进程并管理空闲 TTL 的运行时管理器。
pub(crate) struct AgentRuntimeManager {
    runtimes: Mutex<HashMap<String, Arc<ManagedAgentRuntime>>>,
}

struct ManagedAgentRuntime {
    key: String,
    runtime: Mutex<AgentRuntime>,
    active_sessions: AtomicUsize,
    generation: AtomicU64,
    healthy: AtomicBool,
    idle_timeout: Duration,
}

/// 一个共享 Agent 运行时的 session 租约。
pub(crate) struct AgentRuntimeLease {
    managed: Arc<ManagedAgentRuntime>,
    request_timeout: Duration,
    released: bool,
}

impl AgentRuntimeManager {
    fn global() -> &'static Self {
        static MANAGER: OnceLock<AgentRuntimeManager> = OnceLock::new();
        MANAGER.get_or_init(|| Self {
            runtimes: Mutex::new(HashMap::new()),
        })
    }

    /// 取得或创建同一命令身份的共享 Agent 运行时。
    pub(crate) async fn acquire(
        options: &JdbcAgentOptions,
    ) -> Result<AgentRuntimeLease, DruidError> {
        let manager = Self::global();
        let key = options.runtime_key();
        let mut runtimes = manager.runtimes.lock().await;
        let managed = if let Some(existing) = runtimes
            .get(&key)
            .filter(|runtime| runtime.healthy.load(Ordering::Acquire))
        {
            Arc::clone(existing)
        } else {
            let runtime = AgentRuntime::start(options).await?;
            let managed = Arc::new(ManagedAgentRuntime {
                key: key.clone(),
                runtime: Mutex::new(runtime),
                active_sessions: AtomicUsize::new(0),
                generation: AtomicU64::new(0),
                healthy: AtomicBool::new(true),
                idle_timeout: options.runtime_idle_timeout(),
            });
            runtimes.insert(key, Arc::clone(&managed));
            managed
        };
        managed.active_sessions.fetch_add(1, Ordering::AcqRel);
        managed.generation.fetch_add(1, Ordering::AcqRel);
        Ok(AgentRuntimeLease {
            managed,
            request_timeout: options.timeout(),
            released: false,
        })
    }

    async fn evict_if_idle(managed: Arc<ManagedAgentRuntime>, generation: u64) {
        tokio::time::sleep(managed.idle_timeout).await;
        if managed.active_sessions.load(Ordering::Acquire) != 0
            || managed.generation.load(Ordering::Acquire) != generation
        {
            return;
        }
        let manager = Self::global();
        let mut runtimes = manager.runtimes.lock().await;
        if runtimes
            .get(&managed.key)
            .is_some_and(|current| Arc::ptr_eq(current, &managed))
        {
            runtimes.remove(&managed.key);
        }
    }
}

impl AgentRuntimeLease {
    /// 在共享运行时上执行一次有界请求。
    pub(crate) async fn request(
        &self,
        method: &str,
        params: JsonValue,
    ) -> Result<JsonValue, DruidError> {
        if !self.managed.healthy.load(Ordering::Acquire) {
            return Err(DruidError::ConnectionDiscarded);
        }
        let operation = async {
            let mut runtime = self.managed.runtime.lock().await;
            runtime.request(method, params).await
        };
        match tokio::time::timeout(self.request_timeout, operation).await {
            Ok(result) => {
                if matches!(result, Err(DruidError::DriverError(_))) {
                    self.managed.healthy.store(false, Ordering::Release);
                }
                result
            }
            Err(_) => {
                self.managed.healthy.store(false, Ordering::Release);
                Err(DruidError::DriverError(format!(
                    "JDBC Agent operation '{method}' exceeded {:?}",
                    self.request_timeout
                )))
            }
        }
    }

    /// 返回共享运行时是否已经不可用。
    pub(crate) fn is_unusable(&self) -> bool {
        !self.managed.healthy.load(Ordering::Acquire)
    }

    /// 提前释放 session 租约并启动空闲 TTL；重复调用无副作用。
    pub(crate) fn release_now(&mut self) {
        self.release();
    }

    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let previous = self.managed.active_sessions.fetch_sub(1, Ordering::AcqRel);
        if previous != 1 {
            return;
        }
        let managed = Arc::clone(&self.managed);
        let generation = managed.generation.load(Ordering::Acquire);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(AgentRuntimeManager::evict_if_idle(managed, generation));
        }
    }
}

impl Drop for AgentRuntimeLease {
    fn drop(&mut self) {
        self.release();
    }
}
