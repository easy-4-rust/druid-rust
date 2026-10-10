use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::resource::ResourceOperations;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
fn fatal_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(77, "blob driver failure").with_sql_state("08006"),
    ))
}
fn array_nonfatal_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(88, "array nonfatal failure").with_sql_state("22000"),
    ))
}
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::resource::Array;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};

#[derive(Debug)]
struct ArrayProbe {
    supported: ResourceOperations,
    free_failures: AtomicUsize,
    free_fatal: AtomicBool,
    nonfatal_read: AtomicBool,
    args: (Option<String>, Option<Vec<RdbcObject>>),
    maps: Mutex<Vec<RdbcTypeMap>>,
    calls: Mutex<Vec<&'static str>>,
    frees: AtomicUsize,
    started: tokio::sync::Notify,
    finish: tokio::sync::Notify,
    block: bool,
    read_failure: AtomicBool,
    bad_metadata: AtomicBool,
    block_cursor: AtomicBool,
    cursor_started: tokio::sync::Notify,
    cursor_finish: tokio::sync::Notify,
    cursor_next_failure: AtomicBool,
    cursors: Mutex<Vec<Arc<ArrayCursorProbe>>>,
}
#[derive(Debug, Default)]
struct ArrayCursorProbe {
    closes: AtomicUsize,
    closed: AtomicBool,
}
#[derive(Debug)]
struct ArrayDriver {
    probe: Arc<ArrayProbe>,
    freed: bool,
    not_sync: std::cell::Cell<usize>,
}
impl ArrayDriver {
    fn check(&self, name: &'static str) -> Result<(), RdbcError> {
        if self.freed {
            return Err(RdbcError::rdbc_resource_closed("Array"));
        }
        self.not_sync.set(self.not_sync.get() + 1);
        self.probe.calls.lock().unwrap().push(name);
        let mut required = if name.starts_with("result_set") {
            ResourceOperations::RESULT_SET
        } else {
            ResourceOperations::READ
        };
        if name.contains("range") {
            required |= ResourceOperations::RANGE;
        }
        if name.contains("type_map") {
            required |= ResourceOperations::TYPE_MAP;
        }
        if !self.probe.supported.contains(required) {
            return Err(RdbcError::feature_not_supported(name));
        }
        if self.probe.nonfatal_read.load(Ordering::SeqCst) {
            return Err(array_nonfatal_error());
        }
        if self.probe.read_failure.load(Ordering::SeqCst) {
            return Err(fatal_error());
        }
        Ok(())
    }
    fn range(&self, index: i64, count: i32) -> Result<Vec<(i64, RdbcObject)>, RdbcError> {
        let start = index
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("Array position is 1-based".into()))?;
        let count = usize::try_from(count)
            .map_err(|_| RdbcError::InvalidArgument("negative count".into()))?;
        let values = self.probe.args.1.as_deref().unwrap_or(&[]);
        let tail = values
            .get(start..)
            .ok_or_else(|| RdbcError::InvalidArgument("Array range outside value".into()))?;
        Ok(tail
            .iter()
            .take(count)
            .cloned()
            .enumerate()
            .map(|(i, value)| (index + i as i64, value))
            .collect())
    }
    fn all(&self) -> Vec<(i64, RdbcObject)> {
        self.probe
            .args
            .1
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, value)| (i as i64 + 1, value))
            .collect()
    }
    fn cursor(&self, rows: Vec<(i64, RdbcObject)>) -> Box<dyn ResultSet> {
        let probe = Arc::new(ArrayCursorProbe::default());
        self.probe.cursors.lock().unwrap().push(probe.clone());
        Box::new(ArrayCursor {
            rows,
            offset: 0,
            on_row: false,
            probe,
            parent: self.probe.clone(),
        })
    }
    fn map(&self, map: &RdbcTypeMap) {
        self.probe.maps.lock().unwrap().push(map.clone());
    }
}
#[async_trait::async_trait]
impl Array for ArrayDriver {
    fn capabilities(&self) -> ResourceOperations {
        self.probe.supported
    }
    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        self.check("base_type_name")?;
        Ok(self
            .probe
            .args
            .0
            .clone()
            .unwrap_or_else(|| "vendor.default".into()))
    }
    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        self.check("base_type")?;
        Ok(4)
    }
    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        self.check("values")?;
        Ok(self.all().into_iter().map(|(_, value)| value).collect())
    }
    async fn values_with_type_map(
        &mut self,
        map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.check("values_with_type_map")?;
        self.map(map);
        Ok(self.all().into_iter().map(|(_, v)| v).collect())
    }
    async fn values_range(&mut self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.check("values_range")?;
        Ok(self
            .range(index, count)?
            .into_iter()
            .map(|(_, v)| v)
            .collect())
    }
    async fn values_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.check("values_range_with_type_map")?;
        self.map(map);
        Ok(self
            .range(index, count)?
            .into_iter()
            .map(|(_, v)| v)
            .collect())
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.check("result_set")?;
        self.probe.cursor_started.notify_one();
        if self.probe.block_cursor.load(Ordering::SeqCst) {
            self.probe.cursor_finish.notified().await;
        }
        Ok(self.cursor(self.all()))
    }
    async fn result_set_with_type_map(
        &mut self,
        map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.check("result_set_with_type_map")?;
        self.map(map);
        Ok(self.cursor(self.all()))
    }
    async fn result_set_range(
        &mut self,
        index: i64,
        count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.check("result_set_range")?;
        Ok(self.cursor(self.range(index, count)?))
    }
    async fn result_set_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.check("result_set_range_with_type_map")?;
        self.map(map);
        Ok(self.cursor(self.range(index, count)?))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        if self.freed {
            return Ok(());
        }
        self.probe.frees.fetch_add(1, Ordering::SeqCst);
        self.probe.started.notify_one();
        if self.probe.block {
            self.probe.finish.notified().await;
        }
        if self
            .probe
            .free_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(if self.probe.free_fatal.load(Ordering::SeqCst) {
                fatal_error()
            } else {
                array_nonfatal_error()
            });
        }
        self.freed = true;
        Ok(())
    }
}
#[derive(Debug)]
struct ArrayCursor {
    rows: Vec<(i64, RdbcObject)>,
    offset: usize,
    on_row: bool,
    probe: Arc<ArrayCursorProbe>,
    parent: Arc<ArrayProbe>,
}
druid_rdbc::impl_standard_wrapper!(ArrayCursor => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ArrayCursor {
    fn get_statement(
        &self,
    ) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>, RdbcError> {
        Ok(None)
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        if self.parent.cursor_next_failure.load(Ordering::SeqCst) {
            return Err(fatal_error());
        }
        self.on_row = self.offset < self.rows.len();
        if self.on_row {
            self.offset += 1;
        }
        Ok(self.on_row)
    }
    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) || !self.on_row {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let (index, value) = &self.rows[self.offset - 1];
        match column.get() {
            1 => Ok(Value::Int(*index)),
            2 => match value {
                RdbcObject::Scalar(v) => Ok(v.clone()),
                _ => Err(RdbcError::feature_not_supported("ArrayCursor#value")),
            },
            _ => Err(RdbcError::InvalidArgument("two columns".into())),
        }
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        if self.parent.bad_metadata.load(Ordering::SeqCst) {
            return Err(RdbcError::Other("array cursor metadata rejected".into()));
        }
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
            ColumnDescriptor::new("INDEX", "BIGINT", ColumnNullability::NoNulls),
            ColumnDescriptor::new("VALUE", "INTEGER", ColumnNullability::Nullable),
        ])))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.probe.closed.swap(true, Ordering::SeqCst) {
            self.probe.closes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.probe.closed.load(Ordering::SeqCst))
    }
}
fn array_values() -> Vec<RdbcObject> {
    vec![
        Value::Int(7).into(),
        Value::Null.into(),
        Value::Int(9).into(),
    ]
}
fn array_map() -> RdbcTypeMap {
    let mut map = RdbcTypeMap::new();
    map.insert("vendor.Integer", RdbcTargetType::Integer);
    map
}
fn value_array_probe(supported: ResourceOperations, block: bool) -> Arc<ArrayProbe> {
    Arc::new(ArrayProbe {
        supported,
        free_failures: AtomicUsize::new(0),
        free_fatal: AtomicBool::new(false),
        nonfatal_read: AtomicBool::new(false),
        args: (Some("vendor.Integer".into()), Some(array_values())),
        maps: Mutex::new(Vec::new()),
        calls: Mutex::new(Vec::new()),
        frees: AtomicUsize::new(0),
        started: tokio::sync::Notify::new(),
        finish: tokio::sync::Notify::new(),
        block,
        read_failure: AtomicBool::new(false),
        bad_metadata: AtomicBool::new(false),
        block_cursor: AtomicBool::new(false),
        cursor_started: tokio::sync::Notify::new(),
        cursor_finish: tokio::sync::Notify::new(),
        cursor_next_failure: AtomicBool::new(false),
        cursors: Mutex::new(Vec::new()),
    })
}
fn value_array(probe: Arc<ArrayProbe>) -> druid_rdbc::resource::RdbcArray {
    druid_rdbc::resource::ResourceHandleFactory::array(Box::new(ArrayDriver {
        probe,
        freed: false,
        not_sync: std::cell::Cell::new(0),
    }))
}
async fn value_array_operation(
    value: &druid_rdbc::resource::RdbcArray,
    op: usize,
) -> Result<(), RdbcError> {
    let map = array_map();
    match op {
        0 => value.base_type_name().await.map(|_| ()),
        1 => value.base_type().await.map(|_| ()),
        2 => value.values().await.map(|_| ()),
        3 => value.values_with_type_map(&map).await.map(|_| ()),
        4 => value.values_range(1, 2).await.map(|_| ()),
        5 => value
            .values_range_with_type_map(1, 2, &map)
            .await
            .map(|_| ()),
        6 => {
            let mut cursor = value.result_set().await?;
            cursor.close().await
        }
        7 => {
            let mut cursor = value.result_set_with_type_map(&map).await?;
            cursor.close().await
        }
        8 => {
            let mut cursor = value.result_set_range(1, 2).await?;
            cursor.close().await
        }
        9 => {
            let mut cursor = value.result_set_range_with_type_map(1, 2, &map).await?;
            cursor.close().await
        }
        _ => panic!("unknown operation"),
    }
}
#[tokio::test]
async fn task7_array_value_all_capabilities_gate_before_driver_and_after_free() {
    let flags = [
        ResourceOperations::READ,
        ResourceOperations::RANGE,
        ResourceOperations::TYPE_MAP,
        ResourceOperations::RESULT_SET,
        ResourceOperations::FREE,
    ];
    let required = [
        flags[0],
        flags[0],
        flags[0],
        flags[0] | flags[2],
        flags[0] | flags[1],
        flags[0] | flags[1] | flags[2],
        flags[0] | flags[3],
        flags[0] | flags[3] | flags[2],
        flags[0] | flags[1] | flags[3],
        flags[0] | flags[1] | flags[3] | flags[2],
    ];
    for mask in 0..32 {
        let mut supported = ResourceOperations::empty();
        for (bit, flag) in flags.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                supported |= *flag;
            }
        }
        let probe = value_array_probe(
            supported | ResourceOperations::WRITE | ResourceOperations::STREAM,
            false,
        );
        let value = value_array(probe.clone());
        assert_eq!(value.capabilities(), supported);
        for (op, required) in required.iter().enumerate() {
            let before = probe.calls.lock().unwrap().len();
            let result = value_array_operation(&value, op).await;
            if supported.contains(*required) {
                assert!(result.is_ok(), "mask {mask} op {op}");
                assert_eq!(probe.calls.lock().unwrap().len(), before + 1);
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    error.class_name(),
                    "java.sql.SQLFeatureNotSupportedException"
                );
                assert_eq!(probe.calls.lock().unwrap().len(), before);
            }
        }
        if supported.contains(ResourceOperations::FREE) {
            value.free().await.unwrap();
            value.clone().free().await.unwrap();
            assert!(value.is_freed());
            assert_eq!(probe.frees.load(Ordering::SeqCst), 1);
            for op in 0..10 {
                let before = probe.calls.lock().unwrap().len();
                assert!(value_array_operation(&value, op).await.is_err());
                assert_eq!(probe.calls.lock().unwrap().len(), before);
            }
        } else {
            let error = value.free().await.unwrap_err();
            assert_eq!(
                error.class_name(),
                "java.sql.SQLFeatureNotSupportedException"
            );
            assert_eq!(probe.frees.load(Ordering::SeqCst), 0);
            assert!(!value.is_freed());
        }
    }
}
#[tokio::test]
async fn task7_array_value_identity_maps_ranges_and_raw_cursor_ownership() {
    let probe = value_array_probe(ResourceOperations::array(), false);
    let value = value_array(probe.clone());
    let cloned = value.clone();
    assert_eq!(value, cloned);
    assert_eq!(value.resource_id(), cloned.resource_id());
    assert_ne!(
        value,
        value_array(value_array_probe(ResourceOperations::array(), false))
    );
    assert_eq!(value.get_base_type_name().await.unwrap(), "vendor.Integer");
    assert_eq!(value.get_base_type().await.unwrap(), 4);
    assert_eq!(value.get_array().await.unwrap(), array_values());
    assert_eq!(value.values_range(2, 2).await.unwrap(), array_values()[1..]);
    assert!(value.values_range(0, 1).await.is_err());
    assert!(value.values_range(1, -1).await.is_err());
    let map = array_map();
    assert_eq!(
        value.values_with_type_map(&map).await.unwrap(),
        array_values()
    );
    assert_eq!(
        value.values_range_with_type_map(2, 1, &map).await.unwrap(),
        vec![Value::Null.into()]
    );
    let mut cursor = value
        .result_set_range_with_type_map(2, 1, &map)
        .await
        .unwrap();
    assert!(cursor.next().await.unwrap());
    assert_eq!(
        cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(2)
    );
    assert_eq!(
        cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::Null
    );
    assert_eq!(
        *probe.maps.lock().unwrap(),
        vec![map.clone(), map.clone(), map]
    );
    value.free().await.unwrap();
    assert_eq!(
        probe.cursors.lock().unwrap()[0]
            .closes
            .load(Ordering::SeqCst),
        0
    );
    cursor.close().await.unwrap();
    assert_eq!(
        probe.cursors.lock().unwrap()[0]
            .closes
            .load(Ordering::SeqCst),
        1
    );
}
#[tokio::test]
async fn task7_array_value_original_error_and_failed_free_retry() {
    let probe = value_array_probe(ResourceOperations::array(), false);
    let value = value_array(probe.clone());
    probe.nonfatal_read.store(true, Ordering::SeqCst);
    assert_eq!(
        value.values().await.unwrap_err().to_string(),
        array_nonfatal_error().to_string()
    );
    probe.nonfatal_read.store(false, Ordering::SeqCst);
    probe.free_failures.store(1, Ordering::SeqCst);
    assert_eq!(
        value.free().await.unwrap_err().to_string(),
        array_nonfatal_error().to_string()
    );
    assert!(!value.is_freed());
    assert_eq!(value.values().await.unwrap(), array_values());
    value.free().await.unwrap();
    assert!(value.is_freed());
    assert_eq!(probe.frees.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn task7_array_value_cancelled_raw_free_requires_retry_confirmation() {
    let probe = value_array_probe(ResourceOperations::array(), true);
    let value = value_array(probe.clone());
    let task_value = value.clone();
    let task = tokio::spawn(async move { task_value.free().await });
    probe.started.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!value.is_freed());
    let calls = probe.calls.lock().unwrap().len();
    assert!(value.values().await.is_err());
    assert_eq!(probe.calls.lock().unwrap().len(), calls);
    let task_value = value.clone();
    let retry = tokio::spawn(async move { task_value.free().await });
    probe.started.notified().await;
    probe.finish.notify_one();
    retry.await.unwrap().unwrap();
    assert!(value.is_freed());
    value.free().await.unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn task7_array_value_concurrent_clone_free_calls_driver_once() {
    let probe = value_array_probe(ResourceOperations::array(), true);
    let value = value_array(probe.clone());
    let first = value.clone();
    let second = value.clone();
    let a = tokio::spawn(async move { first.free().await });
    probe.started.notified().await;
    let b = tokio::spawn(async move { second.free().await });
    assert!(value.values().await.is_err());
    probe.finish.notify_one();
    a.await.unwrap().unwrap();
    b.await.unwrap().unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst), 1);
    assert!(value.is_freed());
}
#[tokio::test]
async fn task7_array_value_cancelled_read_unlocks_without_consuming_resource() {
    let probe = value_array_probe(ResourceOperations::array(), false);
    probe.block_cursor.store(true, Ordering::SeqCst);
    let value = value_array(probe.clone());
    let child = value.clone();
    let task = tokio::spawn(async move { child.result_set().await.map(|_| ()) });
    probe.cursor_started.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(value.values().await.unwrap(), array_values());
    assert!(!value.is_freed());
    value.free().await.unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst), 1);
}
/// 在驱动入口计数，避免驱动自身closed检查掩盖共享句柄漏检查。
#[derive(Debug)]
struct BoundaryArray {
    inner: ArrayDriver,
    values_calls: Arc<AtomicUsize>,
    panic_once: bool,
}
#[async_trait::async_trait]
impl Array for BoundaryArray {
    fn capabilities(&self) -> ResourceOperations {
        self.inner.capabilities()
    }
    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        self.inner.base_type_name().await
    }
    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        self.inner.base_type().await
    }
    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        self.values_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.values().await
    }
    async fn values_with_type_map(
        &mut self,
        map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.inner.values_with_type_map(map).await
    }
    async fn values_range(&mut self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.inner.values_range(index, count).await
    }
    async fn values_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.inner
            .values_range_with_type_map(index, count, map)
            .await
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.result_set().await
    }
    async fn result_set_with_type_map(
        &mut self,
        map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.result_set_with_type_map(map).await
    }
    async fn result_set_range(
        &mut self,
        index: i64,
        count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner.result_set_range(index, count).await
    }
    async fn result_set_range_with_type_map(
        &mut self,
        index: i64,
        count: i32,
        map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.inner
            .result_set_range_with_type_map(index, count, map)
            .await
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        if self.panic_once {
            self.panic_once = false;
            panic!("controlled driver free panic");
        }
        self.inner.free().await
    }
}
#[tokio::test]
async fn task7_array_value_panicked_free_keeps_unknown_state_until_retry() {
    let probe = value_array_probe(ResourceOperations::array(), false);
    let values_calls = Arc::new(AtomicUsize::new(0));
    let value = druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {
        inner: ArrayDriver {
            probe: probe.clone(),
            freed: false,
            not_sync: std::cell::Cell::new(0),
        },
        values_calls: values_calls.clone(),
        panic_once: true,
    }));
    let child = value.clone();
    let task = tokio::spawn(async move { child.free().await });
    assert!(task.await.unwrap_err().is_panic());
    assert!(!value.is_freed());
    assert!(value.values().await.is_err());
    assert_eq!(values_calls.load(Ordering::SeqCst), 0);
    value.free().await.unwrap();
    assert!(value.is_freed());
    assert_eq!(probe.frees.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn task7_array_value_waiting_read_rechecks_release_after_lock_handoff() {
    let probe = value_array_probe(ResourceOperations::array(), false);
    probe.block_cursor.store(true, Ordering::SeqCst);
    let values_calls = Arc::new(AtomicUsize::new(0));
    let value = druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {
        inner: ArrayDriver {
            probe: probe.clone(),
            freed: false,
            not_sync: std::cell::Cell::new(0),
        },
        values_calls: values_calls.clone(),
        panic_once: false,
    }));
    let child = value.clone();
    let cursor_task = tokio::spawn(async move { child.result_set().await });
    probe.cursor_started.notified().await;
    let mut release = Box::pin(value.free());
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(release.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    let mut read = Box::pin(value.values());
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(read.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    let calls = probe.calls.lock().unwrap().len();
    probe.cursor_finish.notify_one();
    let mut cursor = cursor_task.await.unwrap().unwrap();
    release.await.unwrap();
    assert!(read.await.is_err());
    assert_eq!(probe.calls.lock().unwrap().len(), calls);
    assert_eq!(probe.frees.load(Ordering::SeqCst), 1);
    assert_eq!(values_calls.load(Ordering::SeqCst), 0);
    cursor.close().await.unwrap();
}
