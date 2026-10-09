use druid_rdbc::resource::Array;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::types::{RdbcObject, RdbcTypeMap, RdbcTargetType, Value};

#[derive(Debug)]
struct ArrayProbe {
    args: (Option<String>, Option<Vec<RdbcObject>>),
    maps: Mutex<Vec<RdbcTypeMap>>,
    calls: Mutex<Vec<&'static str>>,
    frees: AtomicUsize,
    started: tokio::sync::Notify,
    finish: tokio::sync::Notify,
    block: bool,
    read_failure: AtomicBool,
    bad_metadata: AtomicBool,
    cursor_next_failure: AtomicBool,
    cursors: Mutex<Vec<Arc<ArrayCursorProbe>>>,
}
#[derive(Debug, Default)]
struct ArrayCursorProbe { closes: AtomicUsize, closed: AtomicBool }
#[derive(Debug)]
struct ArrayDriver {
    probe: Arc<ArrayProbe>,
    freed: bool,
    not_sync: std::cell::Cell<usize>,
}
impl ArrayDriver {
    fn check(&self, name: &'static str) -> Result<(), RdbcError> {
        if self.freed { return Err(RdbcError::rdbc_resource_closed("Array")); }
        self.not_sync.set(self.not_sync.get() + 1);
        self.probe.calls.lock().unwrap().push(name);
        if self.probe.read_failure.load(Ordering::SeqCst) { return Err(fatal_error()); }
        Ok(())
    }
    fn range(&self, index: i64, count: i32) -> Result<Vec<(i64,RdbcObject)>, RdbcError> {
        let start = index.checked_sub(1).and_then(|i| usize::try_from(i).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("Array position is 1-based".into()))?;
        let count = usize::try_from(count).map_err(|_| RdbcError::InvalidArgument("negative count".into()))?;
        let values = self.probe.args.1.as_deref().unwrap_or(&[]);
        let tail = values.get(start..).ok_or_else(|| RdbcError::InvalidArgument("Array range outside value".into()))?;
        Ok(tail.iter().take(count).cloned().enumerate().map(|(i,value)| (index+i as i64,value)).collect())
    }
    fn all(&self) -> Vec<(i64,RdbcObject)> {
        self.probe.args.1.as_deref().unwrap_or(&[]).iter().cloned().enumerate()
            .map(|(i,value)| (i as i64+1,value)).collect()
    }
    fn cursor(&self, rows: Vec<(i64,RdbcObject)>) -> Box<dyn ResultSet> {
        let probe = Arc::new(ArrayCursorProbe::default());
        self.probe.cursors.lock().unwrap().push(probe.clone());
        Box::new(ArrayCursor { rows, offset: 0, on_row: false, probe, parent: self.probe.clone() })
    }
    fn map(&self, map: &RdbcTypeMap) { self.probe.maps.lock().unwrap().push(map.clone()); }
}
#[async_trait::async_trait]
impl Array for ArrayDriver {
    fn capabilities(&self) -> ResourceOperations { ResourceOperations::array() }
    async fn base_type_name(&mut self) -> Result<String,RdbcError> {
        self.check("base_type_name")?; Ok(self.probe.args.0.clone().unwrap_or_else(|| "vendor.default".into()))
    }
    async fn base_type(&mut self) -> Result<i32,RdbcError> { self.check("base_type")?; Ok(4) }
    async fn values(&mut self) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values")?; Ok(self.all().into_iter().map(|(_,value)| value).collect())
    }
    async fn values_with_type_map(&mut self, map: &RdbcTypeMap) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_with_type_map")?; self.map(map); Ok(self.all().into_iter().map(|(_,v)| v).collect())
    }
    async fn values_range(&mut self, index:i64, count:i32) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_range")?; Ok(self.range(index,count)?.into_iter().map(|(_,v)| v).collect())
    }
    async fn values_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_range_with_type_map")?; self.map(map); Ok(self.range(index,count)?.into_iter().map(|(_,v)| v).collect())
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set")?; Ok(self.cursor(self.all()))
    }
    async fn result_set_with_type_map(&mut self,map:&RdbcTypeMap) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_with_type_map")?; self.map(map); Ok(self.cursor(self.all()))
    }
    async fn result_set_range(&mut self,index:i64,count:i32) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_range")?; Ok(self.cursor(self.range(index,count)?))
    }
    async fn result_set_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_range_with_type_map")?; self.map(map); Ok(self.cursor(self.range(index,count)?))
    }
    async fn free(&mut self) -> Result<(),RdbcError> {
        if self.freed { return Ok(()); }
        self.probe.frees.fetch_add(1,Ordering::SeqCst); self.probe.started.notify_one();
        if self.probe.block { self.probe.finish.notified().await; }
        self.freed = true; Ok(())
    }
}
#[derive(Debug)]
struct ArrayCursor { rows:Vec<(i64,RdbcObject)>, offset:usize, on_row:bool, probe:Arc<ArrayCursorProbe>, parent:Arc<ArrayProbe> }
druid_rdbc::impl_standard_wrapper!(ArrayCursor => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ArrayCursor {
    fn get_statement(&self) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>,RdbcError> { Ok(None) }
    async fn next(&mut self) -> Result<bool,RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) { return Err(RdbcError::ConnectionDiscarded); }
        if self.parent.cursor_next_failure.load(Ordering::SeqCst) { return Err(fatal_error()); }
        self.on_row = self.offset < self.rows.len();
        if self.on_row { self.offset += 1; } Ok(self.on_row)
    }
    fn value(&self,column:ColumnIndex) -> Result<Value,RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) || !self.on_row { return Err(RdbcError::ConnectionDiscarded); }
        let (index,value) = &self.rows[self.offset-1];
        match column.get() {
            1 => Ok(Value::Int(*index)),
            2 => match value { RdbcObject::Scalar(v) => Ok(v.clone()), _ => Err(RdbcError::feature_not_supported("ArrayCursor#value")) },
            _ => Err(RdbcError::InvalidArgument("two columns".into())),
        }
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>,RdbcError> {
        if self.parent.bad_metadata.load(Ordering::SeqCst) { return Err(RdbcError::Other("array cursor metadata rejected".into())); }
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
            ColumnDescriptor::new("INDEX","BIGINT",ColumnNullability::NoNulls),
            ColumnDescriptor::new("VALUE","INTEGER",ColumnNullability::Nullable),
        ])))
    }
    async fn close(&mut self) -> Result<(),RdbcError> {
        if !self.probe.closed.swap(true,Ordering::SeqCst) { self.probe.closes.fetch_add(1,Ordering::SeqCst); }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool,RdbcError> { Ok(self.probe.closed.load(Ordering::SeqCst)) }
}
fn array_values() -> Vec<RdbcObject> { vec![Value::Int(7).into(),Value::Null.into(),Value::Int(9).into()] }
fn array_probe(control:&Control) -> Arc<ArrayProbe> { control.arrays.lock().unwrap().last().unwrap().clone() }
fn array_map() -> RdbcTypeMap { let mut map=RdbcTypeMap::new(); map.insert("vendor.Integer",RdbcTargetType::Integer); map }

#[tokio::test]
async fn task7_array_factory_preserves_nullable_input_and_all_value_overloads() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.expect("pooled Array factory");
    assert_eq!(array.base_type_name().await.unwrap(),"vendor.Integer"); assert_eq!(array.base_type().await.unwrap(),4);
    assert_eq!(array.values().await.unwrap(),array_values()); let map=array_map();
    assert_eq!(array.values_with_type_map(&map).await.unwrap(),array_values());
    assert_eq!(array.values_range(2,99).await.unwrap(),array_values()[1..]);
    assert_eq!(array.values_range_with_type_map(2,1,&map).await.unwrap(),vec![Value::Null.into()]);
    assert!(array.values_range(0,1).await.is_err()); assert!(array.values_range(1,-1).await.is_err());
    assert_eq!(array_probe(&control).maps.lock().unwrap().as_slice(),&[map.clone(),map]);
    let mut null=connection.create_array_of(None,None).await.unwrap(); assert!(null.values().await.unwrap().is_empty());
    let mut empty=connection.create_array_of(Some(""),Some(vec![])).await.unwrap(); assert!(empty.values().await.unwrap().is_empty());
    let probes=control.arrays.lock().unwrap(); assert_eq!(probes[1].args,(None,None)); assert_eq!(probes[2].args,(Some("".into()),Some(vec![]))); drop(probes);
    array.free().await.unwrap(); assert!(array.base_type().await.is_err()); null.free().await.unwrap(); empty.free().await.unwrap(); connection.close().await.unwrap();
}
#[tokio::test]
async fn task7_array_four_cursor_overloads_have_no_creating_statement_and_parent_cleanup() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.unwrap(); let map=array_map();
    let mut cursors=vec![array.result_set().await.unwrap(),array.result_set_with_type_map(&map).await.unwrap(),
        array.result_set_range(2,1).await.unwrap(),array.result_set_range_with_type_map(3,1,&map).await.unwrap()];
    for (i,cursor) in cursors.iter_mut().enumerate() {
        assert!(cursor.get_statement().unwrap().is_none()); assert_eq!(cursor.meta_data().unwrap().column_count(),2);
        assert!(cursor.next().await.unwrap()); assert_eq!(cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),Value::Int(if i<2 {1} else {i as i64}));
        assert_eq!(cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),if i==2 {Value::Null} else if i==3 {Value::Int(9)} else {Value::Int(7)});
    }
    let probe=array_probe(&control); connection.close().await.unwrap();
    assert_eq!(control.returns.lock().unwrap().len(),1); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
    for raw in probe.cursors.lock().unwrap().iter() { assert_eq!(raw.closes.load(Ordering::SeqCst),1); }
    for cursor in &mut cursors { assert!(cursor.next().await.is_err()); cursor.close().await.unwrap(); }
    array.free().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_free_cancellation_remains_owned_until_parent_returns() {
    let control=Arc::new(Control::default()); control.block_free.store(true,Ordering::SeqCst);
    let mut connection=pooled(control.clone()).await; let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap();
    let probe=array_probe(&control); let mut release=Box::pin(array.free());
    tokio::select! { _=probe.started.notified()=>{}, result=&mut release=>panic!("free finished before release: {result:?}") }
    drop(release); let mut parent=Box::pin(connection.close());
    assert!(tokio::time::timeout(Duration::from_millis(30),&mut parent).await.is_err());
    assert!(control.returns.lock().unwrap().is_empty()); probe.finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2),&mut parent).await.unwrap().unwrap(); drop(parent);
    array.free().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(control.returns.lock().unwrap().len(),1);
}
#[tokio::test]
async fn task7_array_cursor_open_failure_does_not_deadlock_parent_cleanup() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    probe.bad_metadata.store(true,Ordering::SeqCst);
    let result=tokio::time::timeout(Duration::from_secs(2),array.result_set()).await.expect("handoff cleanup must finish");
    assert!(result.is_err()); tokio::time::timeout(Duration::from_secs(2),connection.close()).await.unwrap().unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_cursor_driver_fatal_error_discards_actual_owner_once() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    let mut cursor=array.result_set().await.unwrap(); probe.cursor_next_failure.store(true,Ordering::SeqCst);
    let error=cursor.next().await.unwrap_err(); assert_eq!(error.sql_exception().unwrap().sql_state(),Some("08006"));
    assert_eq!(control.discards.load(Ordering::SeqCst),1); assert!(array.values().await.is_err());
    array.free().await.unwrap(); connection.close().await.unwrap(); assert_eq!(control.discards.load(Ordering::SeqCst),1);
}
