use super::{TestCallableConnection, TestCallableState};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::StatementOptions;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
/// 创建真实 BASE 标准 Callable 记录连接。对应 Java: com.alibaba.druid.pool.DruidDataSource。
pub(crate) struct TestCallableFactory {
    prepare_count: Arc<AtomicU64>,
    prepared_options: Arc<Mutex<Vec<(String, StatementOptions)>>>,
    states: Arc<Mutex<Vec<Arc<TestCallableState>>>>,
}
#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for TestCallableFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(TestCallableConnection {
            prepare_count: Arc::clone(&self.prepare_count),
            prepared_options: Arc::clone(&self.prepared_options),
            states: Arc::clone(&self.states),
            closed: false,
        }))
    }
}
/// 创建 BASE 缓存配置的池，返回真实 raw 准备和委托观测。
pub(crate) async fn callable_overload_pool() -> (
    druid::pool::DruidPool,
    Arc<AtomicU64>,
    Arc<Mutex<Vec<(String, StatementOptions)>>>,
    Arc<Mutex<Vec<Arc<TestCallableState>>>>,
) {
    let prepare_count = Arc::new(AtomicU64::new(0));
    let prepared_options = Arc::new(Mutex::new(Vec::new()));
    let states = Arc::new(Mutex::new(Vec::new()));
    let pool = druid::pool::DruidPool::builder()
        .name("callable-overloads")
        .db_type_name("mysql")
        .factory(Arc::new(TestCallableFactory {
            prepare_count: Arc::clone(&prepare_count),
            prepared_options: Arc::clone(&prepared_options),
            states: Arc::clone(&states),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .unwrap();
    (pool, prepare_count, prepared_options, states)
}
