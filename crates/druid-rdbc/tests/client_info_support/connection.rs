//! 有真实属性状态及失败注入的标准连接测试后端。
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, SqlException, SqlWarning};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 可观察整组替换、部分失败和取消的客户端信息连接。对应 Java: java.sql.Connection。
pub struct ClientInfoConnection {
    pub values: Arc<Mutex<HashMap<String, String>>>,
    pub fail_name: Arc<Mutex<Option<String>>>,
    pub snapshot_supported: Arc<AtomicBool>,
    pub snapshot_calls: Arc<AtomicUsize>,
    pub writes: Arc<AtomicUsize>,
    pub closed: Arc<AtomicBool>,
    pub pause_next_write: Arc<AtomicBool>,
    pub started: Arc<tokio::sync::Notify>,
    pub release: Arc<tokio::sync::Notify>,
}
impl ClientInfoConnection {
    /// 参数为初始属性，返回拥有独立驱动状态的连接；对应 Java: Connection。
    pub fn new(values: HashMap<String, String>) -> Self {
        Self {
            values: Arc::new(Mutex::new(values)),
            fail_name: Arc::new(Mutex::new(None)),
            snapshot_supported: Arc::new(AtomicBool::new(true)),
            snapshot_calls: Arc::new(AtomicUsize::new(0)),
            writes: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicBool::new(false)),
            pause_next_write: Arc::new(AtomicBool::new(false)),
            started: Arc::new(tokio::sync::Notify::new()),
            release: Arc::new(tokio::sync::Notify::new()),
        }
    }
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                9,
                Some("08003".to_owned()),
                Some("connection is closed".to_owned()),
            ))));
        }
        Ok(())
    }
}
druid_rdbc::impl_standard_wrapper!(ClientInfoConnection => Connection);
#[async_trait::async_trait]
impl Connection for ClientInfoConnection {
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(true)
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(false)
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::SeqCst))
    }
    fn client_info(&self, name: &str) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.values.lock().unwrap().get(name).cloned())
    }
    async fn client_info_properties(&mut self) -> Result<HashMap<String, String>, RdbcError> {
        self.ensure_open()?;
        self.snapshot_calls.fetch_add(1, Ordering::SeqCst);
        if !self.snapshot_supported.load(Ordering::SeqCst) {
            return Err(RdbcError::feature_not_supported(
                "test_client_info_snapshot",
            ));
        }
        Ok(self.values.lock().unwrap().clone())
    }
    async fn set_client_info(&mut self, name: &str, value: Option<&str>) -> Result<(), RdbcError> {
        self.ensure_open().map_err(|error| {
            error.with_client_info_failure(HashMap::from([(
                name.to_owned(),
                "REASON_UNKNOWN".to_owned(),
            )]))
        })?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail_name.lock().unwrap().as_deref() == Some(name) {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                77,
                Some("22023".to_owned()),
                Some("client property rejected".to_owned()),
            )))
            .with_client_info_failure(HashMap::from([(
                name.to_owned(),
                "REASON_VALUE_INVALID".to_owned(),
            )])));
        }
        {
            let mut properties = self.values.lock().unwrap();
            match value {
                Some(value) => {
                    properties.insert(name.to_owned(), value.to_owned());
                }
                None => {
                    properties.remove(name);
                }
            }
        }
        if self.pause_next_write.swap(false, Ordering::SeqCst) {
            self.started.notify_one();
            self.release.notified().await;
        }
        Ok(())
    }
}
