/// 在驱动入口计数，避免驱动自身closed检查掩盖共享句柄漏检查。
#[derive(Debug)]
struct BoundaryArray {inner:ArrayDriver,values_calls:Arc<AtomicUsize>,panic_once:bool}
#[async_trait::async_trait]
impl Array for BoundaryArray {
    fn capabilities(&self)->ResourceOperations {self.inner.capabilities()}
    async fn base_type_name(&mut self)->Result<String,RdbcError>{self.inner.base_type_name().await}
    async fn base_type(&mut self)->Result<i32,RdbcError>{self.inner.base_type().await}
    async fn values(&mut self)->Result<Vec<RdbcObject>,RdbcError>{self.values_calls.fetch_add(1,Ordering::SeqCst);self.inner.values().await}
    async fn values_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_with_type_map(map).await}
    async fn values_range(&mut self,index:i64,count:i32)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_range(index,count).await}
    async fn values_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_range_with_type_map(index,count,map).await}
    async fn result_set(&mut self)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set().await}
    async fn result_set_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_with_type_map(map).await}
    async fn result_set_range(&mut self,index:i64,count:i32)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_range(index,count).await}
    async fn result_set_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_range_with_type_map(index,count,map).await}
    async fn free(&mut self)->Result<(),RdbcError>{if self.panic_once {self.panic_once=false;panic!("controlled driver free panic");}self.inner.free().await}
}
#[tokio::test]
async fn task7_array_value_panicked_free_keeps_unknown_state_until_retry() {
    let probe=value_array_probe(ResourceOperations::array(),false);let values_calls=Arc::new(AtomicUsize::new(0));
    let value=druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {inner:ArrayDriver {probe:probe.clone(),freed:false,not_sync:std::cell::Cell::new(0)},values_calls:values_calls.clone(),panic_once:true}));
    let child=value.clone();let task=tokio::spawn(async move{child.free().await});assert!(task.await.unwrap_err().is_panic());assert!(!value.is_freed());assert!(value.values().await.is_err());assert_eq!(values_calls.load(Ordering::SeqCst),0);
    value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
