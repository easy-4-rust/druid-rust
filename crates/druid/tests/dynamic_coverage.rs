//! Dynamic datasource coverage tests.
use druid::core::*;
use druid::dynamic::*;
use std::sync::Arc;
use std::time::Duration;

struct MockPool;
#[async_trait::async_trait]
impl Pool for MockPool {
    async fn get(&self) -> Result<PooledConnection, DruidError> {
        let conn = Box::new(MockConn) as Box<dyn Connection>;
        Ok(PooledConnection::new(conn, 1, Box::new(|_, _| {})))
    }
    async fn get_timeout(&self, _: Duration) -> Result<PooledConnection, DruidError> {
        self.get().await
    }
    fn state(&self) -> PoolState {
        PoolState {
            name: "mock".into(),
            ..Default::default()
        }
    }
    fn driver_name(&self) -> &str {
        "mock"
    }
    fn name(&self) -> &str {
        "mock"
    }
}

struct MockConn;
#[async_trait::async_trait]
impl Connection for MockConn {
    async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![])
    }
    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
}

#[test]
fn test_round_robin_balancer() {
    let s1 = Arc::new(MockPool) as Arc<dyn Pool>;
    let s2 = Arc::new(MockPool) as Arc<dyn Pool>;
    let lb = RoundRobinBalancer::new();
    let pools = vec![s1.clone(), s2.clone()];
    assert!(lb.pick(&pools).is_some());
    assert_eq!(lb.name(), "round_robin");
}

#[test]
fn test_round_robin_empty_pools() {
    let lb = RoundRobinBalancer::new();
    let pools: Vec<Arc<dyn Pool>> = vec![];
    assert!(lb.pick(&pools).is_none());
}

#[test]
fn test_random_balancer() {
    let s1 = Arc::new(MockPool) as Arc<dyn Pool>;
    let lb = RandomBalancer;
    let pools = vec![s1];
    assert!(lb.pick(&pools).is_some());
    assert_eq!(lb.name(), "random");
}

#[test]
fn test_random_balancer_empty() {
    let lb = RandomBalancer;
    assert!(lb.pick(&[]).is_none());
}

#[test]
fn test_sql_hint_equality() {
    assert_eq!(SqlHint::Read, SqlHint::Read);
    assert_ne!(SqlHint::Read, SqlHint::Write);
}

#[test]
fn test_datasource_group_new() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let slave = Arc::new(MockPool) as Arc<dyn Pool>;
    let group = DataSourceGroup::new(
        "main",
        master,
        vec![slave],
        Arc::new(RoundRobinBalancer::new()),
    );
    assert_eq!(group.name, "main");
    assert_eq!(group.slaves.len(), 1);
}

#[test]
fn test_dynamic_datasource_create() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("main", master, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);
    assert_eq!(ds.current_name(), "main");
}

#[tokio::test]
async fn test_dynamic_datasource_route_write() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("main", master, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Write);
    let mut conn = pool.get().await.unwrap();
    conn.ping().await.unwrap();
}

#[tokio::test]
async fn test_dynamic_datasource_route_read_no_slaves() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("main", master, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Read);
    assert_eq!(pool.name(), "mock"); // fallback to master
}

#[tokio::test]
async fn test_dynamic_datasource_route_auto() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let group = DataSourceGroup::new("main", master, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Auto);
    assert_eq!(pool.name(), "mock");
}

#[tokio::test]
async fn test_dynamic_datasource_hot_switch() {
    let master1 = Arc::new(MockPool) as Arc<dyn Pool>;
    let slave1 = Arc::new(MockPool) as Arc<dyn Pool>;
    let g1 = DataSourceGroup::new(
        "v1",
        master1,
        vec![slave1],
        Arc::new(RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(g1);
    assert_eq!(ds.current_name(), "v1");

    let master2 = Arc::new(MockPool) as Arc<dyn Pool>;
    let g2 = DataSourceGroup::new("v2", master2, vec![], Arc::new(RoundRobinBalancer::new()));
    ds.switch(g2);
    assert_eq!(ds.current_name(), "v2");
}

#[tokio::test]
async fn test_dynamic_datasource_route_read_with_slaves() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let slave = Arc::new(MockPool) as Arc<dyn Pool>;
    let lb = Arc::new(RoundRobinBalancer::new()) as Arc<dyn LoadBalancer>;
    let group = DataSourceGroup::new("main", master, vec![slave], lb);
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Read);
    // Should pick from slaves
    let mut conn = pool.get().await.unwrap();
    conn.ping().await.unwrap();
}

#[tokio::test]
async fn test_dynamic_datasource_current() {
    let master = Arc::new(MockPool) as Arc<dyn Pool>;
    let g = DataSourceGroup::new("main", master, vec![], Arc::new(RoundRobinBalancer::new()));
    let ds = DynamicDataSource::new(g);
    let current = ds.current();
    assert_eq!(current.name, "main");
}
