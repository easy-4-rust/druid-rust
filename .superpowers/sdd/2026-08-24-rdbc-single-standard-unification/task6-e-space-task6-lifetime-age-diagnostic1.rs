use druid::core::{Connection,ConnectionFactory,ExecResult,Row,Value};
use druid::pool::DruidPool;
use druid_rdbc::error::RdbcError;
use std::sync::{Arc,atomic::{AtomicU64,Ordering}};
use std::time::{Duration,Instant};
#[derive(Debug)]
struct Driver {closed:bool,closes:Arc<AtomicU64>}
druid_rdbc::impl_standard_wrapper!(Driver => Connection);
#[async_trait::async_trait]
impl Connection for Driver {
 async fn exec(&mut self,_sql:&str,_params:Vec<Value>)->Result<ExecResult,RdbcError>{Ok(ExecResult::default())}
 async fn fetch(&mut self,_sql:&str,_params:Vec<Value>)->Result<Vec<Row>,RdbcError>{Ok(Vec::new())}
 async fn begin(&mut self)->Result<(),RdbcError>{Ok(())}
 async fn commit(&mut self)->Result<(),RdbcError>{Ok(())}
 async fn rollback(&mut self)->Result<(),RdbcError>{Ok(())}
 async fn ping(&mut self)->Result<(),RdbcError>{Ok(())}
 async fn close(&mut self)->Result<(),RdbcError>{if !self.closed {self.closed=true;self.closes.fetch_add(1,Ordering::SeqCst);}Ok(())}
 fn is_closed(&self)->Result<bool,RdbcError>{Ok(self.closed)}
 fn driver_name(&self)->Result<String,RdbcError>{Ok("fault-test".into())}
}
struct Factory {creates:Arc<AtomicU64>,closes:Arc<AtomicU64>}
#[async_trait::async_trait]
impl ConnectionFactory for Factory {
 async fn create(&self)->Result<Box<dyn Connection>,RdbcError>{self.creates.fetch_add(1,Ordering::SeqCst);Ok(Box::new(Driver{closed:false,closes:self.closes.clone()}))}
 async fn validate(&self,connection:&mut Box<dyn Connection>)->Result<(),RdbcError>{connection.ping().await}
}
async fn configured(lifetime:Duration)->(DruidPool,Arc<Factory>){
 let factory=Arc::new(Factory{creates:Arc::new(AtomicU64::new(0)),closes:Arc::new(AtomicU64::new(0))});
 let pool=DruidPool::builder().name("lifetime-diagnostic").driver_name("fault-test").factory(factory.clone()).max_open(4).max_idle(4).acquire_timeout(Duration::from_millis(200)).max_lifetime(lifetime).min_idle(0).build().await.unwrap();(pool,factory)
}
#[tokio::test]
async fn lifetime_original_first_idle_assertion_with_observed_age() {
 let (pool,_factory)=configured(Duration::from_millis(10)).await;
 let mut connection=pool.get().await.unwrap();
 let holder=connection.connection_holder().unwrap();let created=holder.created_at;let id=holder.connection_id();
 if std::env::var_os("TASK6_LIFETIME_INJECT_BEFORE_CLOSE").is_some(){tokio::time::sleep(Duration::from_millis(30)).await;}
 let age_before=created.elapsed();connection.close().await.unwrap();let age_after=created.elapsed();let state=pool.state();
 eprintln!("AGE_OBSERVATION id={} before_ns={} after_ns={} max_ns=10000000 idle={} active={} discard={} destroy={} recycle={} close={}",id,age_before.as_nanos(),age_after.as_nanos(),state.idle_count,state.active_count,state.discard_count,state.destroy_count,state.recycle_count,state.close_count);
 assert_eq!(state.idle_count,1);tokio::time::sleep(Duration::from_millis(30)).await;let next=pool.get().await.unwrap();assert_eq!(pool.state().active_count,1);drop(next);pool.close().await;
}
#[tokio::test]
async fn lifetime_control_known_expired_holder_is_discarded_without_leak() {
 let (pool,factory)=configured(Duration::from_millis(10)).await;
 let mut connection=pool.get().await.unwrap();let holder=connection.connection_holder_mut().unwrap();holder.created_at=Instant::now()-Duration::from_secs(1);let id=holder.connection_id();assert!(holder.connection_age()>=Duration::from_millis(10));
 connection.close().await.unwrap();let state=pool.state();assert_eq!(state.idle_count,0);assert_eq!(state.active_count,0);assert_eq!(state.discard_count,1);assert_eq!(state.destroy_count,1);assert_eq!(state.close_count,1);assert_eq!(state.recycle_count,0);assert_eq!(factory.closes.load(Ordering::SeqCst),1);
 let mut next=pool.get().await.unwrap();assert_ne!(next.connection_holder().unwrap().connection_id(),id);assert_eq!(factory.creates.load(Ordering::SeqCst),2);next.close().await.unwrap();pool.close().await;
}
#[tokio::test]
async fn lifetime_control_nonexpired_holder_is_reused() {
 let (pool,factory)=configured(Duration::from_secs(3600)).await;let mut connection=pool.get().await.unwrap();let id=connection.connection_holder().unwrap().connection_id();connection.close().await.unwrap();let state=pool.state();assert_eq!(state.idle_count,1);assert_eq!(state.active_count,0);assert_eq!(state.discard_count,0);assert_eq!(state.destroy_count,0);assert_eq!(state.recycle_count,1);let mut next=pool.get().await.unwrap();assert_eq!(next.connection_holder().unwrap().connection_id(),id);assert_eq!(factory.creates.load(Ordering::SeqCst),1);next.close().await.unwrap();pool.close().await;
}
