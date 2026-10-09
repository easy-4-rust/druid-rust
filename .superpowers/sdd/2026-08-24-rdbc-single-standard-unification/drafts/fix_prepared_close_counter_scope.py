from pathlib import Path
p=Path("crates/druid-wrapper/tests/task6_prepared_statement_canonical_matrix_test.rs")
s=p.read_text(encoding="utf-8")
old="struct CleanupFactory(CleanupFailure, Option<Arc<AtomicUsize>>);"
new="""/// 每个实际底层连接使用独立关闭计数；第一个连接保留原测试观察器。
struct CleanupFactory {
    failure: CleanupFailure,
    first_raw_close_count: Option<Arc<AtomicUsize>>,
    raw_close_counts: Mutex<Vec<Arc<AtomicUsize>>>,
}

impl CleanupFactory {
    /// 保存故障注入与原连接观察器，返回可记录补建连接的工厂。
    fn new(failure: CleanupFailure, first_raw_close_count: Option<Arc<AtomicUsize>>) -> Self {
        Self {
            failure,
            first_raw_close_count,
            raw_close_counts: Mutex::new(Vec::new()),
        }
    }

    /// 池关闭后逐个检查实际连接释放次数，不能由补建连接污染原连接计数。
    fn assert_all_connections_closed_once(&self, created: u64) {
        let counts = self.raw_close_counts.lock().unwrap();
        assert_eq!(counts.len(), usize::try_from(created).unwrap());
        assert!(!counts.is_empty());
        for (id, count) in counts.iter().enumerate() {
            assert_eq!(count.load(Ordering::Acquire), 1, "raw connection {id} must close once");
        }
    }
}"""
assert old in s;s=s.replace(old,new,1)
old="""        Ok(Box::new(CleanupConnection {
            failure: self.0,
            closed: false,
            raw_close_count: self.1.clone(),
        }))"""
new="""        let raw_close_count = {
            let mut counts = self.raw_close_counts.lock().unwrap();
            let count = if counts.is_empty() {
                self.first_raw_close_count.clone().unwrap_or_else(|| Arc::new(AtomicUsize::new(0)))
            } else {
                Arc::new(AtomicUsize::new(0))
            };
            counts.push(Arc::clone(&count));
            count
        };
        Ok(Box::new(CleanupConnection {
            failure: self.failure,
            closed: false,
            raw_close_count: Some(raw_close_count),
        }))"""
assert old in s;s=s.replace(old,new,1)
s=s.replace("CleanupFactory(failure, None)","CleanupFactory::new(failure, None)")
old="""    let raw_close_count = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()"""
new="""    let raw_close_count = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(CleanupFactory::new(failure, Some(Arc::clone(&raw_close_count))));
    let pool = DruidPool::builder()"""
assert old in s;s=s.replace(old,new,1)
old="""        .factory(Arc::new(CleanupFactory(
            failure,
            Some(Arc::clone(&raw_close_count)),
        )))"""
assert old in s;s=s.replace(old,"        .factory(factory.clone())",1)
old="""    // 连接归还只提交关闭任务；pool shutdown 才等待受监管 worker 完成。
    pool.close().await;
    assert_eq!(raw_close_count.load(Ordering::Acquire), 1);"""
new="""    // 显式关闭等待原连接完成；shutdown 还必须等待实际补建连接关闭。
    pool.close().await;
    assert_eq!(raw_close_count.load(Ordering::Acquire), 1);
    factory.assert_all_connections_closed_once(pool.state().create_count);"""
assert old in s;s=s.replace(old,new,1)
old="""    let classifications = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()"""
new="""    let classifications = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(CleanupFactory::new(failure, Some(raw_closes.clone())));
    let pool = DruidPool::builder()"""
assert old in s;s=s.replace(old,new,1)
old="        .factory(Arc::new(CleanupFactory(failure, Some(raw_closes.clone()))))"
assert old in s;s=s.replace(old,"        .factory(factory.clone())",1)
old="""    assert_eq!(raw_closes.load(Ordering::Acquire), 1);
    assert!(handle.is_closed().unwrap(), "registry 最终必须完成逻辑关闭");"""
new="""    assert_eq!(raw_closes.load(Ordering::Acquire), 1);
    factory.assert_all_connections_closed_once(pool.state().create_count);
    assert!(handle.is_closed().unwrap(), "registry 最终必须完成逻辑关闭");"""
assert old in s;s=s.replace(old,new,1)
p.write_text(s,encoding="utf-8")
print("Preserved first raw close=1 assertions; added separate checks for every actual replacement connection.")
