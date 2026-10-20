//! 对象、描述符和流读取的同一路径错误计数回归。
mod standard_test_support;
use druid::core::{
    DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, ResultSetObjectRead,
    SqlException,
};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::{RdbcObject, RdbcTargetType, Value};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// 对应 Java: ResultSet Filter 的对象、读取与资源重载；保留同一个 SQL 异常。
struct ReadFailure(Arc<AtomicUsize>);
impl ReadFailure {
    fn reject<T>(&self) -> Result<T, DruidError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Err(DruidError::SqlException(Box::new(SqlException::new(
            73,
            Some("HY111".to_owned()),
            Some("read rejected".to_owned()),
        ))))
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for ReadFailure {
    fn standard_result_set_object(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetObjectRead<'_>,
        _next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        self.reject()
    }
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        _next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        self.reject()
    }
    fn standard_result_set_resource(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        _next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        self.reject()
    }
    fn standard_result_set_input_stream(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _kind: ResultSetStreamKind,
        _next: &mut dyn FnMut()
            -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError>,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError> {
        self.reject()
    }
    fn standard_result_set_reader(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _national: bool,
        _next: &mut dyn FnMut() -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError>,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError> {
        self.reject()
    }
}

fn read(rows: &dyn ResultSet, operation: usize) -> Result<(), RdbcError> {
    let column = ColumnIndex::new(1).unwrap();
    match operation {
        0 => rows.object(column).map(|_| ()),
        1 => rows.object_by_label("1").map(|_| ()),
        2 => rows.object_with_type_map(column, None).map(|_| ()),
        3 => rows.object_with_type_map_by_label("1", None).map(|_| ()),
        4 => rows.object_as(column, &RdbcTargetType::String).map(|_| ()),
        5 => rows
            .object_as_by_label("1", &RdbcTargetType::String)
            .map(|_| ()),
        6 => rows.read_object(column, &ResultSetRead::String).map(|_| ()),
        7 => rows
            .read_object_by_label("1", &ResultSetRead::String)
            .map(|_| ()),
        8 => rows.read_object(column, &ResultSetRead::Blob).map(|_| ()),
        9 => rows
            .read_object_by_label("1", &ResultSetRead::Blob)
            .map(|_| ()),
        10..=15 => {
            let kind = match (operation - 10) / 2 {
                0 => ResultSetStreamKind::Ascii,
                1 => ResultSetStreamKind::Binary,
                _ => ResultSetStreamKind::Unicode,
            };
            if operation.is_multiple_of(2) {
                rows.input_stream(column, kind).map(|_| ())
            } else {
                rows.input_stream_by_label("1", kind).map(|_| ())
            }
        }
        16..=19 => {
            if operation.is_multiple_of(2) {
                rows.reader(column, operation >= 18).map(|_| ())
            } else {
                rows.reader_by_label("1", operation >= 18).map(|_| ())
            }
        }
        _ => panic!("unknown operation"),
    }
}

#[tokio::test]
async fn every_read_route_counts_the_original_error_once_without_new_execution() {
    let mut executed = 0;
    for prepared in [false, true] {
        let state = TrackingState::configured();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(ReadFailure(Arc::clone(&calls))));
        let pool = DruidPool::new_standard(
            format!("read-error-{prepared}"),
            "mock".to_owned(),
            Arc::new(TrackingFactory::new(Arc::clone(&state))),
            PoolInnerConfig {
                max_open: 1,
                max_idle: 1,
                ..PoolInnerConfig::default()
            },
            Some(Arc::new(filters)),
        );
        pool.init().await.unwrap();
        let mut connection = pool.get().await.unwrap();
        let (mut statement, mut rows): (Box<dyn druid_rdbc::statement::Statement>, _) = if prepared
        {
            let mut statement = connection
                .prepare_statement("select 1", StatementOptions::default())
                .await
                .unwrap();
            let rows = statement.execute_prepared_query().await.unwrap();
            (statement, rows)
        } else {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await
                .unwrap();
            let rows = statement.execute_query("select 1").await.unwrap();
            (statement, rows)
        };
        assert_eq!(pool.stat_value_and_reset().error_count, 0);
        for operation in 0..20 {
            let before = pool.stats_collector().result_set_stat().error_count();
            let error = read(rows.as_ref(), operation).unwrap_err();
            let sql = error.sql_exception().unwrap();
            assert_eq!(sql.error_code(), 73);
            assert_eq!(sql.sql_state(), Some("HY111"));
            assert_eq!(sql.message(), Some("read rejected"));
            assert_eq!(
                pool.stats_collector().result_set_stat().error_count(),
                before + 1,
                "result-set error once: {operation}"
            );
            let snapshot = pool.stat_value_and_reset();
            assert_eq!(
                snapshot.error_count, 1,
                "data-source error once: {operation}"
            );
            assert_eq!(snapshot.execute_count, 0, "read error cannot execute SQL");
            assert_eq!(calls.load(Ordering::Acquire), operation + 1);
            assert!(!rows.is_closed().unwrap());
            executed += 1;
        }
        assert!(rows.next().await.unwrap());
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        let mut next = pool.get().await.unwrap();
        assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
        next.close().await.unwrap();
        pool.close().await;
    }
    assert_eq!(executed, 40);
}
