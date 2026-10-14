use super::{AgentRuntime, JdbcAgentOptions, JdbcAgentRuntimeMetrics};
use druid_rdbc::error::RdbcError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// 按 Agent 命令身份共享子进程并管理空闲 TTL 的运行时管理器。
pub(crate) struct AgentRuntimeManager {
    runtimes: Mutex<HashMap<String, Arc<ManagedAgentRuntime>>>,
    startup_locks: StdMutex<HashMap<String, Arc<Mutex<()>>>>,
}

struct StartupLockLease {
    key: String,
    lock: Arc<Mutex<()>>,
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

impl AgentRuntimeManager {
    fn global() -> &'static Self {
        static MANAGER: OnceLock<AgentRuntimeManager> = OnceLock::new();
        MANAGER.get_or_init(|| Self {
            runtimes: Mutex::new(HashMap::new()),
            startup_locks: StdMutex::new(HashMap::new()),
        })
    }

    fn startup_lock(&self, key: &str) -> StartupLockLease {
        let mut locks = self
            .startup_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let lock = Arc::clone(
            locks
                .entry(key.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        );
        StartupLockLease {
            key: key.to_owned(),
            lock,
        }
    }

    /// 取得或创建同一命令身份的共享 Agent 运行时。
    pub(crate) async fn acquire(
        options: &JdbcAgentOptions,
    ) -> Result<AgentRuntimeLease, RdbcError> {
        let manager = Self::global();
        let key = options.runtime_key();
        let startup_deadline = tokio::time::Instant::now() + options.timeout();
        // 同一身份只启动一次；启动等待不能占用其他身份共用的映射锁。
        let startup_lock = manager.startup_lock(&key);
        let _startup_guard = tokio::time::timeout_at(startup_deadline, startup_lock.lock.lock())
            .await
            .map_err(|_| {
                RdbcError::DriverError(format!(
                    "JDBC Agent startup timed out after {:?}",
                    options.timeout()
                ))
            })?;
        let mut runtimes = manager.runtimes.lock().await;
        let managed = if let Some(existing) = runtimes
            .get(&key)
            .filter(|runtime| runtime.healthy.load(Ordering::Acquire))
        {
            let managed = Arc::clone(existing);
            managed.active_sessions.fetch_add(1, Ordering::AcqRel);
            managed.generation.fetch_add(1, Ordering::AcqRel);
            managed
        } else {
            drop(runtimes);
            let healthy = Arc::new(AtomicBool::new(true));
            // ready 与握手都属于启动；不可信子进程不能无限占用本身份的启动锁。
            let runtime = tokio::time::timeout_at(
                startup_deadline,
                AgentRuntime::start(options, Arc::clone(&healthy)),
            )
            .await
            .map_err(|_| {
                RdbcError::DriverError(format!(
                    "JDBC Agent startup timed out after {:?}",
                    options.timeout()
                ))
            })??;
            let managed = Arc::new(ManagedAgentRuntime {
                key: key.clone(),
                runtime: Arc::new(runtime),
                active_sessions: AtomicUsize::new(1),
                generation: AtomicU64::new(1),
                healthy,
                idle_timeout: options.runtime_idle_timeout(),
            });
            runtimes = manager.runtimes.lock().await;
            runtimes.insert(key, Arc::clone(&managed));
            managed
        };
        JdbcAgentRuntimeMetrics::session_opened();
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

impl Drop for StartupLockLease {
    fn drop(&mut self) {
        let mut locks = AgentRuntimeManager::global()
            .startup_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Arc::strong_count(&self.lock) == 2
            && locks
                .get(&self.key)
                .is_some_and(|current| Arc::ptr_eq(current, &self.lock))
        {
            locks.remove(&self.key);
        }
    }
}

impl AgentRuntimeLease {
    /// 在共享运行时上执行一次有界请求。
    pub(crate) async fn request(
        &self,
        method: &str,
        params: JsonValue,
    ) -> Result<JsonValue, RdbcError> {
        if !self.managed.healthy.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
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
        if let Ok(result) = tokio::time::timeout(self.request_timeout, pending.wait()).await {
            JdbcAgentRuntimeMetrics::rpc_completed(started.elapsed(), result.is_err());
            result
        } else {
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
            Err(RdbcError::DriverError(format!(
                "JDBC Agent operation '{method}' requestId={request_id} exceeded {:?}; cancellation requested",
                self.request_timeout,
            )))
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

impl Drop for AgentRuntimeLease {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn silent_agent_startup_obeys_request_timeout() {
        #[cfg(windows)]
        let options = JdbcAgentOptions::new("powershell.exe")
            .argument("-NoProfile")
            .argument("-Command")
            .argument("Start-Sleep -Seconds 10")
            .runtime_identity("silent-ready", "unmanaged", "ready-test")
            .request_timeout(Duration::from_millis(75));
        #[cfg(not(windows))]
        let options = JdbcAgentOptions::new("sh")
            .argument("-c")
            .argument("sleep 10")
            .runtime_identity("silent-ready", "unmanaged", "ready-test")
            .request_timeout(Duration::from_millis(75));
        let key = options.runtime_key();

        let result = tokio::time::timeout(
            Duration::from_secs(2),
            AgentRuntimeManager::acquire(&options),
        )
        .await
        .expect("a silent Agent must not hold the runtime manager indefinitely");
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("startup timed out")),
            "startup must report its configured timeout"
        );
        assert!(
            !AgentRuntimeManager::global()
                .startup_locks
                .lock()
                .expect("startup lock map")
                .contains_key(&key),
            "failed startup must not retain its lock entry"
        );
    }

    #[tokio::test]
    async fn agent_that_never_answers_handshake_obeys_startup_timeout() {
        #[cfg(windows)]
        let options = JdbcAgentOptions::new("powershell.exe")
            .argument("-NoProfile")
            .argument("-Command")
            .argument("[Console]::Out.WriteLine('{\"jsonrpc\":\"2.0\",\"method\":\"ready\",\"params\":{\"protocolVersion\":1}}'); [Console]::In.ReadLine() | Out-Null; Start-Sleep -Seconds 10")
            .runtime_identity("silent-handshake", "unmanaged", "handshake-test")
            .request_timeout(Duration::from_millis(100));
        #[cfg(not(windows))]
        let options = JdbcAgentOptions::new("sh")
            .argument("-c")
            .argument("printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"ready\",\"params\":{\"protocolVersion\":1}}'; IFS= read -r request; sleep 10")
            .runtime_identity("silent-handshake", "unmanaged", "handshake-test")
            .request_timeout(Duration::from_millis(100));
        let key = options.runtime_key();

        let result = tokio::time::timeout(
            Duration::from_secs(2),
            AgentRuntimeManager::acquire(&options),
        )
        .await
        .expect("an Agent that stalls after ready must not block startup indefinitely");
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("startup timed out")),
            "a stalled handshake must report the configured startup timeout"
        );
        assert!(
            !AgentRuntimeManager::global()
                .startup_locks
                .lock()
                .expect("startup lock map")
                .contains_key(&key),
            "failed handshake must release its startup lock"
        );
    }

    #[tokio::test]
    async fn silent_agent_does_not_block_unrelated_runtime_identity() {
        #[cfg(windows)]
        let stalled = JdbcAgentOptions::new("powershell.exe")
            .argument("-NoProfile")
            .argument("-Command")
            .argument("Start-Sleep -Seconds 10")
            .runtime_identity("silent-independent", "unmanaged", "independent-test")
            .request_timeout(Duration::from_secs(2));
        #[cfg(not(windows))]
        let stalled = JdbcAgentOptions::new("sh")
            .argument("-c")
            .argument("sleep 10")
            .runtime_identity("silent-independent", "unmanaged", "independent-test")
            .request_timeout(Duration::from_secs(2));
        let stalled_key = stalled.runtime_key();

        let stalled_task =
            tokio::spawn(async move { AgentRuntimeManager::acquire(&stalled).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !stalled_task.is_finished(),
            "fixture must still be starting"
        );

        let unrelated = JdbcAgentOptions::new("nonexistent-druid-agent-executable")
            .request_timeout(Duration::from_millis(200));
        let result = tokio::time::timeout(
            Duration::from_millis(700),
            AgentRuntimeManager::acquire(&unrelated),
        )
        .await
        .expect("a different Agent identity must not wait for stalled startup");
        assert!(matches!(result, Err(RdbcError::DriverError(_))));
        stalled_task.abort();
        let _ = stalled_task.await;
        assert!(
            !AgentRuntimeManager::global()
                .startup_locks
                .lock()
                .expect("startup lock map")
                .contains_key(&stalled_key),
            "cancelled startup must not retain its lock entry"
        );
    }

    #[tokio::test]
    async fn waiting_for_same_agent_startup_obeys_own_timeout() {
        #[cfg(windows)]
        let stalled = JdbcAgentOptions::new("powershell.exe")
            .argument("-NoProfile")
            .argument("-Command")
            .argument("Start-Sleep -Seconds 10")
            .runtime_identity("silent-same", "unmanaged", "same-test")
            .request_timeout(Duration::from_secs(2));
        #[cfg(not(windows))]
        let stalled = JdbcAgentOptions::new("sh")
            .argument("-c")
            .argument("sleep 10")
            .runtime_identity("silent-same", "unmanaged", "same-test")
            .request_timeout(Duration::from_secs(2));
        let impatient = stalled.clone().request_timeout(Duration::from_millis(75));
        let stalled_task =
            tokio::spawn(async move { AgentRuntimeManager::acquire(&stalled).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !stalled_task.is_finished(),
            "fixture must still be starting"
        );

        let result = tokio::time::timeout(
            Duration::from_millis(700),
            AgentRuntimeManager::acquire(&impatient),
        )
        .await
        .expect("same-identity startup wait must be bounded by this caller's timeout");
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("startup timed out"))
        );
        stalled_task.abort();
        let _ = stalled_task.await;
    }
}
