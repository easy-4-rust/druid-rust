//! druid-dynamic S5 验收测试

use druid_core::*;
use druid_dynamic::*;
use druid_pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct MockConnection { closed: bool, exec_count: Arc<AtomicU64> }
impl MockConnection {
    fn new() -> (Self, Arc<AtomicU64>) {
        let c = Arc::new(AtomicU64::new(0));
        (Self { closed: false, exec_count: c.clone() }, c)
    }
}

#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn exec(&mut self, _sql: &str, _p: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.exec_count.fetch_add(1, Ordering::Relaxed);
        Ok(ExecResult { rows_affected: 1, last_insert_id: None })
    }
    async fn fetch(&mut self, _sql: &str, _p: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
    async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn close(&mut self) -> Result<(), DruidError> { self.closed = true; Ok(()) }
    fn driver_name(&self) -> &str { "mock" }
}

struct MockFactory;
#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        let (c, _) = MockConnection::new();
        Ok(Box::new(c))
    }
    async fn validate(&self, c: &mut Box<dyn Connection>) -> Result<(), DruidError> { c.ping().await }
}

async fn make_pool(name: &str) -> DruidPool {
    DruidPool::builder()
        .name(name).driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(2).max_idle(2)
        .build().await.unwrap()
}

#[tokio::test]
async fn test_dynamic_datasource_create() {
    let master = Arc::new(make_pool("master").await) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("main", master.clone(), vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);
    assert_eq!(ds.current_name(), "main");
}

#[tokio::test]
async fn test_route_write_goes_to_master() {
    let master = Arc::new(make_pool("master").await) as Arc<dyn Pool>;
    let slave = Arc::new(make_pool("slave").await) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("v1", master.clone(), vec![slave], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);

    let pool = ds.route(SqlHint::Write);
    assert_eq!(pool.name(), "master");
}

#[tokio::test]
async fn test_route_read_goes_to_slave() {
    let master = Arc::new(make_pool("master").await) as Arc<dyn Pool>;
    let slave = Arc::new(make_pool("slave-1").await) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("v1", master.clone(), vec![slave], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);

    let pool = ds.route(SqlHint::Read);
    assert_eq!(pool.name(), "slave-1");
}

#[tokio::test]
async fn test_route_read_fallback_to_master_when_no_slaves() {
    let master = Arc::new(make_pool("master").await) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("v1", master.clone(), vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);

    let pool = ds.route(SqlHint::Read);
    assert_eq!(pool.name(), "master");
}

#[tokio::test]
async fn test_hot_switch() {
    let master_v1 = Arc::new(make_pool("v1-master").await) as Arc<dyn Pool>;
    let group_v1 = DataSourceGroup::new("v1", master_v1, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group_v1);
    assert_eq!(ds.current_name(), "v1");

    let master_v2 = Arc::new(make_pool("v2-master").await) as Arc<dyn Pool>;
    let group_v2 = DataSourceGroup::new("v2", master_v2, vec![], Arc::new(RoundRobinBalancer::new()));
    ds.switch(group_v2);

    assert_eq!(ds.current_name(), "v2");
    let pool = ds.route(SqlHint::Write);
    assert_eq!(pool.name(), "v2-master");
}

#[tokio::test]
async fn test_round_robin_balancer() {
    let s1 = Arc::new(make_pool("s1").await) as Arc<dyn Pool>;
    let s2 = Arc::new(make_pool("s2").await) as Arc<dyn Pool>;
    let s3 = Arc::new(make_pool("s3").await) as Arc<dyn Pool>;
    let lb = RoundRobinBalancer::new();
    let pools = vec![s1, s2, s3];

    assert_eq!(lb.pick(&pools).unwrap().name(), "s1");
    assert_eq!(lb.pick(&pools).unwrap().name(), "s2");
    assert_eq!(lb.pick(&pools).unwrap().name(), "s3");
    assert_eq!(lb.pick(&pools).unwrap().name(), "s1"); // wrap around
}

#[test]
fn test_random_balancer() {
    // Just verify it doesn't panic with empty pools
    let lb = RandomBalancer;
    assert!(lb.pick(&[]).is_none());
}

#[test]
fn test_load_balancer_name() {
    assert_eq!(RoundRobinBalancer::new().name(), "round_robin");
    assert_eq!(RandomBalancer.name(), "random");
}

#[test]
fn test_sql_hint_equality() {
    assert_eq!(SqlHint::Read, SqlHint::Read);
    assert_ne!(SqlHint::Read, SqlHint::Write);
}
