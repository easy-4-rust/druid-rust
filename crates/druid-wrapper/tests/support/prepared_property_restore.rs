//! 保留 BASE prepared_statement_semantics_test 的属性恢复顺序与首错停止证据。

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{FetchDirection, PreparedStatement, Statement, StatementOptions};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 对应 BASE PreparedPropertyProbe，记录真实池化代理对底层属性的访问顺序。
pub(crate) struct PropertyProbe {
    pub(crate) events: Mutex<Vec<String>>,
    pub(crate) fail_restore_once: AtomicBool,
    pub(crate) fail_capture_once: AtomicBool,
    pub(crate) fail_close_once: AtomicBool,
    pub(crate) prepare_count: AtomicUsize,
}

impl PropertyProbe {
    fn record(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }

    pub(crate) fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

/// 对应 BASE PreparedPropertyStatement，提供非零初始值及一次恢复失败。
struct PropertyStatement {
    probe: Arc<PropertyProbe>,
    max_field_size: i32,
    max_rows: i64,
    query_timeout: i32,
    fetch_direction: FetchDirection,
    fetch_size: i32,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(PropertyStatement => PreparedStatement);

macro_rules! property {
    ($get:ident, $set:ident, $ty:ty) => {
        fn $get(&self) -> Result<$ty, RdbcError> {
            self.probe.record(concat!("get_", stringify!($get)));
            Ok(self.$get)
        }
        fn $set(&mut self, value: $ty) -> Result<(), RdbcError> {
            self.probe.record(format!("{}:{value}", stringify!($set)));
            self.$get = value;
            Ok(())
        }
    };
}

#[async_trait::async_trait]
impl Statement for PropertyStatement {
    property!(max_field_size, set_max_field_size, i32);
    property!(query_timeout, set_query_timeout, i32);
    property!(fetch_size, set_fetch_size, i32);

    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.probe.record("get_max_rows");
        if self.probe.fail_capture_once.swap(false, Ordering::AcqRel) {
            return Err(RdbcError::DriverError("capture max rows failed".to_owned()));
        }
        Ok(self.max_rows)
    }

    fn set_max_rows(&mut self, value: i64) -> Result<(), RdbcError> {
        self.probe.record(format!("set_max_rows:{value}"));
        if value == 12 && self.probe.fail_restore_once.swap(false, Ordering::AcqRel) {
            return Err(RdbcError::DriverError("restore max rows failed".to_owned()));
        }
        self.max_rows = value;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.probe.record("get_fetch_direction");
        Ok(self.fetch_direction)
    }

    fn set_fetch_direction(&mut self, value: FetchDirection) -> Result<(), RdbcError> {
        self.probe.record(format!("set_fetch_direction:{value:?}"));
        self.fetch_direction = value;
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.probe.record("clear_batch");
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.probe.record("close");
        if self.probe.fail_close_once.swap(false, Ordering::AcqRel) {
            return Err(RdbcError::DriverError(
                "property statement close failed".to_owned(),
            ));
        }
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for PropertyStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("select 1")
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.probe.record("clear_parameters");
        Ok(())
    }
}

/// 对应 BASE PreparedPropertyConnection，仅负责创建可观察的真实底层语句。
struct PropertyConnection {
    probe: Arc<PropertyProbe>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(PropertyConnection => Connection);

#[async_trait::async_trait]
impl Connection for PropertyConnection {
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.probe.prepare_count.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(PropertyStatement {
            probe: Arc::clone(&self.probe),
            max_field_size: 11,
            max_rows: 12,
            query_timeout: 13,
            fetch_direction: FetchDirection::Reverse,
            fetch_size: 14,
            closed: false,
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

/// 对应 BASE PreparedPropertyFactory，共享探针而不模拟池化代理的恢复逻辑。
pub(crate) struct PropertyFactory(pub(crate) Arc<PropertyProbe>);

#[async_trait::async_trait]
impl ConnectionFactory for PropertyFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(PropertyConnection {
            probe: Arc::clone(&self.0),
            closed: false,
        }))
    }
}
