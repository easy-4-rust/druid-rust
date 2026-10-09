from pathlib import Path
import re

r=Path.cwd();e=r/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
p=r/'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs';existing=p.read_text(encoding='utf-8')
name='real_sqlite_result_sets_flow_through_stat_filter_and_statement_cascade'
assert 'async fn task6_'+name not in existing
s=(e/'drafts/complete_stat_cascade_base_original_20261007.rs.txt').read_text(encoding='utf-8')
s=s.replace('async fn '+name,'async fn task6_'+name)
s=s.replace('() {\n','''() {
    use crate::core::FilterChain;
    use crate::stats::{StatsCollector, StatFilter};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, RdbcTargetType};
    use native_result_set_close_observer::NativeResultSetCloseObserver;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;
''',1)
start=s.index('    let physical = factory');end=s.index('    let mut first = first_statement',start)
s=s[:start]+'''    let (mut first_statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, 72, "sqlite-stat", Arc::new(filter_chain),
    ).await;
'''+s[end:]
s=s.replace('.execute_query_result_set(\n            &mut connection,','.execute_query(')
s=s.replace('first.next(&mut connection).unwrap()','first.next().await.unwrap()').replace('second.next(&mut connection).unwrap()','second.next().await.unwrap()')
s=s.replace('&mut connection, 1','ColumnIndex::new(1).unwrap()').replace('&mut connection, 2','ColumnIndex::new(2).unwrap()')
s=s.replace('.object_typed(','.object_as(')
s=s.replace('Value::Int(1));','RdbcObject::Long(1));')
s=s.replace('first.date(&mut connection, 3)','first.date(ColumnIndex::new(3).unwrap(), &RdbcCalendarArgument::Unspecified)')
s=s.replace('first.time_with_calendar(&mut connection, 4, None)','first.time(ColumnIndex::new(4).unwrap(), &RdbcCalendarArgument::Specified(None))')
s=s.replace('.timestamp_with_calendar(&mut connection, 5, Some(RdbcCalendar::new("UTC").unwrap()),)', '.timestamp(ColumnIndex::new(5).unwrap(), &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap())))')
s=s.replace('first.close_with_connection(&mut connection).unwrap();','first.close().await.unwrap();')
s=s.replace('first_statement\n        .close_with_connection(&mut connection)\n        .unwrap();','first_statement.close().await.unwrap();')
old='    let mut second_statement = connection.create_statement().await.unwrap();';assert s.count(old)==1
new='''    // 第二语句来自同一真实连接，复用同一策略、资源登记和ID种子。
    let admission = first_statement.inner.begin_standard_admission().unwrap();
    let raw = connection.create_statement(StatementOptions::default()).await.unwrap();
    let mut second_statement = DruidPooledStatement::new_standard(
        raw, StatementOptions::default(), Arc::clone(&first_statement.inner.resources),
        Arc::clone(&first_statement.inner.policy), 72, 2,
        Arc::clone(&first_statement.inner.result_set_id_seed),
        Arc::clone(&first_statement.inner.metadata_id_seed),
        Arc::clone(&first_statement.inner.lease_active),
        first_statement.inner.filter_chain.clone(), None,
    );
    first_statement.inner.resources.register(&admission, Arc::clone(&second_statement.inner)).await.unwrap();
    drop(admission);'''
s=s.replace(old,new).replace('.execute_query_result_set(&mut connection, "SELECT 4 UNION ALL SELECT 5")','.execute_query("SELECT 4 UNION ALL SELECT 5")')
before='    assert!(second.next().await.unwrap());';assert s.count(before)==1
observe='''    // 包围原查询登记的同一原生游标，只观察实际关闭，不能合成关闭状态。
    let native_closed = Arc::new(AtomicBool::new(false));
    let native_close_count = Arc::new(AtomicUsize::new(0));
    let resource = second_statement.current_result_set_for_diagnostics();
    {
        let mut slot = resource.result_set.lock().await;
        let original = slot.take().expect("同一真实SQLite查询游标");
        let identity = (original.as_ref() as *const dyn ResultSet).cast::<()>() as usize;
        *slot = Some(Box::new(NativeResultSetCloseObserver::new(
            original, Arc::clone(&native_closed), Arc::clone(&native_close_count), identity,
        )));
    }
'''
s=s.replace(before,observe+before)
s=s.replace('second_statement\n        .close_with_connection(&mut connection)\n        .unwrap();','second_statement.close().await.unwrap();')
s=s.replace('assert!(second.is_closed());','assert!(second.is_closed().unwrap());')
s=s.replace('assert!(second.raw_result_set().is_closed());','''assert!(native_closed.load(Ordering::Acquire));
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    second_statement.close().await.unwrap();
    second.close().await.unwrap();
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    assert_eq!(stat.close_count(), 2);
    connection.close().await.unwrap();''')
assert 'with_connection' not in s and 'execute_query_result_set' not in s
fixture='''use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 观察原查询的同一原生游标关闭状态与次数，不替换执行后端或合成关闭。
/// 对应 Java: java.sql.ResultSet#close/isClosed。
pub(super) struct NativeResultSetCloseObserver {
    inner: Box<dyn ResultSet>,
    closed_observed: Arc<AtomicBool>,
    close_calls: Arc<AtomicUsize>,
    original_identity: usize,
}
impl NativeResultSetCloseObserver {
    /// 参数为原游标、共享关闭观察、计数及原身份，返回仅委托的观察句柄。
    pub(super) fn new(inner: Box<dyn ResultSet>, closed_observed: Arc<AtomicBool>, close_calls: Arc<AtomicUsize>, original_identity: usize) -> Self {
        Self { inner, closed_observed, close_calls, original_identity }
    }
}
druid_rdbc::impl_standard_wrapper!(NativeResultSetCloseObserver => ResultSet);
#[async_trait::async_trait]
impl ResultSet for NativeResultSetCloseObserver {
    fn known_row_count(&self) -> Option<u64> { self.inner.known_row_count() }
    async fn next(&mut self) -> Result<bool, RdbcError> { self.inner.next().await }
    async fn close(&mut self) -> Result<(), RdbcError> {
        assert_eq!((self.inner.as_ref() as *const dyn ResultSet).cast::<()>() as usize, self.original_identity);
        self.close_calls.fetch_add(1, Ordering::AcqRel);
        self.inner.close().await?;
        self.closed_observed.store(self.inner.is_closed()?, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> { self.inner.is_closed() }
}
'''
q=p.parent/'diagnostic_tests/native_result_set_close_observer.rs';assert not q.exists();q.write_bytes(fixture.encode())
p.write_bytes((existing+'\nmod native_result_set_close_observer;\n\n/// 恢复完整原真实SQLite读取、统计与创建语句级联关闭合同。\n'+s).encode())
print('Complete StatFilter/SQLite cascade original restored, including observed native cursor close')
