use super::{AgentRuntime, JdbcAgentOptions, JdbcAgentRuntimeMetrics};
use druid_core::core::DruidError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// 按 Agent 命令身份共享子进程并管理空闲 TTL 的运行时管理器。
pub(crate) struct AgentRuntimeManager {
    runtimes: Mutex<HashMap<String, Arc<ManagedAgentRuntime>>>,
}

struct ManagedAgentRuntime {
    key: String,
    runtime: Arc<AgentRuntime>,
    active_sessions: AtomicUsize,
    generation: AtomicU64,
    healthy: Arc<AtomicBool>,
    idle_timeout: Duration,
}

/// 一个共享 Agent 运行时的 session 租约。
pub(crate) struct AgentRuntimeLease {
    managed: Arc<ManagedAgentRuntime>,
    request_timeout: Duration,
    released: bool,
}

/// 不增加 session 引用计数的运行时请求句柄，供同步 Statement 生命周期回调使用。
#[derive(Clone)]
pub(crate) struct AgentRequestHandle {
    runtime: Arc<AgentRuntime>,
    healthy: Arc<AtomicBool>,
    request_timeout: Duration,
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
            let healthy = Arc::new(AtomicBool::new(true));
            let runtime = AgentRuntime::start(options, Arc::clone(&healthy)).await?;
            let managed = Arc::new(ManagedAgentRuntime {
                key: key.clone(),
                runtime: Arc::new(runtime),
                active_sessions: AtomicUsize::new(0),
                generation: AtomicU64::new(0),
                healthy,
                idle_timeout: options.runtime_idle_timeout(),
            });
            runtimes.insert(key, Arc::clone(&managed));
            managed
        };
        managed.active_sessions.fetch_add(1, Ordering::AcqRel);
        JdbcAgentRuntimeMetrics::session_opened();
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
        let runtime = {
            let mut runtimes = manager.runtimes.lock().await;
            if !runtimes
                .get(&managed.key)
                .is_some_and(|current| Arc::ptr_eq(current, &managed))
                || managed.active_sessions.load(Ordering::Acquire) != 0
                || managed.generation.load(Ordering::Acquire) != generation
            {
                return;
            }
            runtimes.remove(&managed.key);
            Arc::clone(&managed.runtime)
        };
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            runtime.request("shutdown", serde_json::json!({})),
        )
        .await;
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
        let started = Instant::now();
        let session_id = params.get("sessionId").cloned();
        let pending = match self.managed.runtime.begin_request(method, params) {
            Ok(pending) => pending,
            Err(error) => {
                JdbcAgentRuntimeMetrics::rpc_completed(started.elapsed(), true);
                return Err(error);
            }
        };
        let request_id = pending.request_id();
        match tokio::time::timeout(self.request_timeout, pending.wait()).await {
            Ok(result) => {
                JdbcAgentRuntimeMetrics::rpc_completed(started.elapsed(), result.is_err());
                result
            }
            Err(_) => {
                JdbcAgentRuntimeMetrics::rpc_completed(started.elapsed(), true);
                JdbcAgentRuntimeMetrics::request_timed_out();
                JdbcAgentRuntimeMetrics::cancellation_requested();
                let runtime = Arc::clone(&self.managed.runtime);
                let mut cancel_params = serde_json::Map::new();
                cancel_params.insert("targetRequestId".to_owned(), request_id.into());
                if let Some(session_id) = session_id {
                    cancel_params.insert("sessionId".to_owned(), session_id);
                }
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(1),
                        runtime.request("cancel", JsonValue::Object(cancel_params)),
                    )
                    .await;
                });
                Err(DruidError::DriverError(format!(
                    "JDBC Agent operation '{method}' requestId={request_id} exceeded {:?}; cancellation requested",
                    self.request_timeout,
                )))
            }
        }
    }

    /// 返回共享运行时是否已经不可用。
    pub(crate) fn is_unusable(&self) -> bool {
        !self.managed.healthy.load(Ordering::Acquire)
    }

    /// 创建不改变 session 生命周期的轻量请求句柄。
    pub(crate) fn request_handle(&self) -> AgentRequestHandle {
        AgentRequestHandle {
            runtime: Arc::clone(&self.managed.runtime),
            healthy: Arc::clone(&self.managed.healthy),
            request_timeout: self.request_timeout,
        }
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
        JdbcAgentRuntimeMetrics::session_closed();
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

impl AgentRequestHandle {
    /// 在当前 Tokio 运行时中发送尽力而为的异步请求。
    pub(crate) fn spawn_request(&self, method: &'static str, params: JsonValue) {
        if !self.healthy.load(Ordering::Acquire) {
            return;
        }
        let runtime = Arc::clone(&self.runtime);
        let timeout = self.request_timeout;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if method == "cancel" {
                JdbcAgentRuntimeMetrics::cancellation_requested();
            }
            handle.spawn(async move {
                let started = Instant::now();
                let result = tokio::time::timeout(timeout, runtime.request(method, params)).await;
                let failed = !matches!(result, Ok(Ok(_)));
                JdbcAgentRuntimeMetrics::rpc_completed(started.elapsed(), failed);
                if result.is_err() {
                    JdbcAgentRuntimeMetrics::request_timed_out();
                }
            });
        }
    }
}

impl Drop for AgentRuntimeLease {
    fn drop(&mut self) {
        self.release();
    }
}
