//! Canonical owned-PreparedStatement matrix replacing private SqlText/cache test coupling.

#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;

#[path = "support/prepared_property_restore.rs"]
mod prepared_property_restore;

#[path = "support/prepared_execute_recorder.rs"]
mod prepared_execute_recorder;

#[path = "support/prepared_descriptor_recorder.rs"]
mod prepared_descriptor_recorder;

use druid::core::{
    ConnectionFactory, DruidConnectionHolder, ExceptionSorter, ExceptionSorterProperties,
    MySqlExceptionSorter, RdbcError, SqlException,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::resource::RdbcInputStream;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, FetchDirection, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::types::Row;
use druid_rdbc::types::{
    RdbcBigDecimal as BigDecimal, RdbcCalendarArgument, RdbcNaiveDate as NaiveDate,
    RdbcNaiveDateTime as NaiveDateTime, RdbcNaiveTime as NaiveTime, RdbcStreamLength, Value,
};
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use prepared_property_restore::{PropertyFactory, PropertyProbe};
use standard_test_support::{TrackingConnection, TrackingFactory, TrackingState};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
enum CleanupFailure {
    AddBatch,
    ClearBatch,
    ClearParameters,
    SetParameter,
    GetResultSet,
    GetUpdateCount,
    GetGeneratedKeys,
    GetMoreResults,
}

struct CleanupPrepared {
    failure: CleanupFailure,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CleanupPrepared => PreparedStatement);

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for CleanupPrepared {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.execute_prepared_query().await
    }

    async fn get_result_set(&mut self) -> Result<Option<Box<dyn ResultSet>>, RdbcError> {
        if matches!(self.failure, CleanupFailure::GetResultSet) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(None)
        }
    }

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        if matches!(self.failure, CleanupFailure::GetUpdateCount) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(Some(0))
        }
    }

    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        if matches!(self.failure, CleanupFailure::GetGeneratedKeys) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(Box::new(BufferedRowSet::with_column_labels(
                Vec::new(),
                Vec::new(),
            )))
        }
    }

    async fn get_more_results(
        &mut self,
        _disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        if matches!(self.failure, CleanupFailure::GetMoreResults) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(false)
        }
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for CleanupPrepared {
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(BufferedRowSet::with_column_labels(
            vec![Row::new(vec![Value::Int(1)])],
            vec!["value".to_owned()],
        )))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(0)
    }

    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("SELECT 1")
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearParameters) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::SetParameter) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::AddBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        if matches!(self.failure, CleanupFailure::ClearBatch) {
            Err(task6_cleanup_sql_error())
        } else {
            Ok(())
        }
    }
}

fn task6_cleanup_sql_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(1040, "too many connections")
            .with_class_name("com.mysql.cj.rdbc.exceptions.MySQLNonTransientConnectionException"),
    ))
}

struct CleanupConnection {
    failure: CleanupFailure,
    closed: bool,
    raw_close_count: Option<Arc<AtomicUsize>>,
}

druid_rdbc::impl_standard_wrapper!(CleanupConnection => Connection);

#[async_trait::async_trait]
impl Connection for CleanupConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        // BASE raw fetch 返回一行；MySQL 校验通过标准 Statement 读取同一查询行为。
        Ok(Box::new(CleanupPrepared {
            failure: self.failure,
            closed: false,
        }))
    }

    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(CleanupPrepared {
            failure: self.failure,
            closed: false,
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            if let Some(count) = &self.raw_close_count {
                count.fetch_add(1, Ordering::AcqRel);
            }
            self.closed = true;
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

/// 每个实际底层连接使用独立关闭计数；第一个连接保留原测试观察器。
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
            assert_eq!(
                count.load(Ordering::Acquire),
                1,
                "raw connection {id} must close once"
            );
        }
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for CleanupFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let raw_close_count = {
            let mut counts = self.raw_close_counts.lock().unwrap();
            let count = if counts.is_empty() {
                self.first_raw_close_count
                    .clone()
                    .unwrap_or_else(|| Arc::new(AtomicUsize::new(0)))
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
        }))
    }
}

struct CleanupFatalSorter(Option<Arc<AtomicUsize>>);
impl ExceptionSorter for CleanupFatalSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        if let Some(count) = &self.0 {
            assert_eq!(
                exception,
                task6_cleanup_sql_error().sql_exception().unwrap()
            );
            count.fetch_add(1, Ordering::AcqRel);
            // 仅记录真实 sorter 调用；fatal 判断保留 BASE MySQL 分类逻辑。
            return MySqlExceptionSorter.is_exception_fatal(exception);
        }
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

fn task6_prepared_statement_case_ids() -> &'static [&'static str] {
    &[
        "clear_batch_failure_uses_connection_sorter_before_cache_return",
        "clear_parameters_failure_uses_connection_sorter_before_cache_return",
        "concurrent_logical_statements_preserve_java_in_use_and_lru_semantics",
        "direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter",
        "holder_statement_pool_is_lazy_and_debug_does_not_initialize_it",
        "mysql_schema_change_clears_existing_statement_cache",
        "pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle",
        "prepared_batch_snapshots_parameters_and_executes_against_real_sqlite",
        "prepared_bound_setters_execute_and_batch_against_real_sqlite",
        "prepared_close_restores_properties_in_java_order_and_stops_on_first_error",
        "prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics",
        "prepared_generic_execute_preserves_ordered_multi_results",
        "prepared_input_parameter_converts_every_scalar_family_without_losing_type",
        "prepared_result_getters_route_driver_failures_through_connection_sorter",
        "prepared_result_set_keeps_the_same_statement_alive_and_can_close_it",
        "prepared_setter_overloads_preserve_exact_descriptor_identity",
        "prepared_statement_cannot_cross_connection_lease_boundary",
        "prepared_statement_reuses_physical_handle_and_updates_java_stats",
        "prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite",
        "result_set_statement_handle_reports_clear_batch_failure",
        "result_set_statement_handle_reports_clear_parameters_failure",
        "statement_execution_error_removes_handle_instead_of_reusing_it",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).expect("1-based parameter"),
        value,
    }
}

async fn task6_prepared_pool(max_cached: usize) -> (DruidPool, Arc<TrackingState>) {
    let state = TrackingState::configured();
    let pool = DruidPool::builder()
        .name("task6-prepared-parameterized")
        .driver_name("tracking")
        .factory(Arc::new(TrackingFactory::new(Arc::clone(&state))) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_cached)
        .build()
        .await
        .unwrap();
    (pool, state)
}

async fn task6_sqlx_prepared_pool(max_cached: usize) -> DruidPool {
    DruidPool::builder()
        .name("task6-sqlx-prepared-parameterized")
        .driver_name("sqlx")
        .factory(
            Arc::new(SqlxConnectionFactory::new("sqlite::memory:")) as Arc<dyn ConnectionFactory>
        )
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(max_cached)
        .build()
        .await
        .unwrap()
}

async fn task6_pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle(
) {
    let pool = task6_sqlx_prepared_pool(1).await;
    let mut connection = pool.get().await.unwrap();
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );
    let mut prepared = Connection::prepare_statement(&mut connection, "SELECT 1", options)
        .await
        .unwrap();
    assert_eq!(Statement::options(&*prepared).unwrap(), options);
    assert_eq!(
        Statement::options(&*prepared).unwrap().result_set_type(),
        ResultSetType::ScrollInsensitive
    );
    assert_eq!(
        Statement::options(&*prepared).unwrap().concurrency(),
        ResultSetConcurrency::Updatable
    );
    assert_eq!(
        Statement::options(&*prepared).unwrap().holdability(),
        ResultSetHoldability::HoldOverCommit
    );
    assert_eq!(Statement::warnings(&*prepared).unwrap(), None);
    assert_eq!(Statement::max_field_size(&*prepared).unwrap(), 0);
    Statement::set_max_field_size(&mut *prepared, 1024).unwrap();
    assert_eq!(Statement::max_field_size(&*prepared).unwrap(), 1024);
    assert_eq!(Statement::max_rows(&*prepared).unwrap(), 0);
    Statement::set_max_rows(&mut *prepared, 100).unwrap();
    assert_eq!(Statement::max_rows(&*prepared).unwrap(), 100);
    assert_eq!(Statement::query_timeout(&*prepared).unwrap(), 0);
    Statement::set_query_timeout(&mut *prepared, 3).unwrap();
    assert_eq!(Statement::query_timeout(&*prepared).unwrap(), 3);
    assert_eq!(
        Statement::fetch_direction(&*prepared).unwrap(),
        FetchDirection::Unknown
    );
    Statement::set_fetch_direction(&mut *prepared, FetchDirection::Reverse).unwrap();
    assert_eq!(
        Statement::fetch_direction(&*prepared).unwrap(),
        FetchDirection::Reverse
    );
    assert_eq!(Statement::fetch_size(&*prepared).unwrap(), 0);
    Statement::set_fetch_size(&mut *prepared, 25).unwrap();
    assert_eq!(Statement::fetch_size(&*prepared).unwrap(), 25);
    Statement::set_escape_processing(&mut *prepared, true).unwrap();
    Statement::set_cursor_name(&mut *prepared, "prepared_cursor").unwrap();
    Statement::cancel(&mut *prepared).unwrap();
    assert!(!Statement::is_poolable(&*prepared).unwrap());
    Statement::set_poolable(&mut *prepared, true).unwrap();
    assert!(Statement::is_poolable(&*prepared).unwrap());
    Statement::set_poolable(&mut *prepared, false).unwrap();
    assert!(!Statement::is_poolable(&*prepared).unwrap());
    Statement::close_on_completion(&mut *prepared).unwrap();
    assert!(Statement::is_close_on_completion(&*prepared).unwrap());
    Statement::close(&mut *prepared).await.unwrap();
    assert!(Statement::is_closed(&*prepared).unwrap());

    let mut restored =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    assert_eq!(Statement::max_field_size(&*restored).unwrap(), 0);
    assert_eq!(Statement::max_rows(&*restored).unwrap(), 0);
    assert_eq!(Statement::query_timeout(&*restored).unwrap(), 0);
    assert_eq!(
        Statement::fetch_direction(&*restored).unwrap(),
        FetchDirection::Unknown
    );
    assert_eq!(Statement::fetch_size(&*restored).unwrap(), 0);
    Statement::close(&mut *restored).await.unwrap();
    Connection::close(&mut connection).await.unwrap();
    pool.close().await;
}

async fn task6_assert_cleanup_failure_discards_connection(failure: CleanupFailure) {
    let pool = DruidPool::builder()
        .name("task6-prepared-cleanup-failure")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory::new(failure, None)) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(CleanupFatalSorter(None)))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    assert!(matches!(
        statement.close().await,
        Err(RdbcError::SqlException(_))
    ));
    assert!(Connection::is_closed(&connection).unwrap());
    assert!(!connection.is_recycled());
    Connection::close(&mut connection).await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

async fn task6_assert_direct_prepared_method_failure_discards_connection(failure: CleanupFailure) {
    let pool = DruidPool::builder()
        .name("task6-prepared-direct-failure")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory::new(failure, None)) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(CleanupFatalSorter(None)))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    let result = match failure {
        CleanupFailure::SetParameter => {
            PreparedStatement::set_parameter(&mut *statement, parameter(1, Value::Int(1)))
        }
        CleanupFailure::AddBatch => PreparedStatement::add_parameter_batch(
            &mut *statement,
            vec![parameter(1, Value::Int(1))],
        ),
        CleanupFailure::ClearParameters => PreparedStatement::clear_parameters(&mut *statement),
        CleanupFailure::ClearBatch => PreparedStatement::clear_prepared_batch(&mut *statement),
        _ => unreachable!("getter variants use dedicated tests"),
    };
    assert!(matches!(result, Err(RdbcError::SqlException(_))));
    assert!(Connection::is_closed(&connection).unwrap());
    assert!(!connection.is_recycled());
    Connection::close(&mut connection).await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

/// 恢复 BASE getter 在成功执行之后触发的驱动错误、分类与丢弃合同。
async fn task6_assert_prepared_getter_failure_discards_connection(failure: CleanupFailure) {
    let sorter_calls = Arc::new(AtomicUsize::new(0));
    let raw_close_count = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(CleanupFactory::new(
        failure,
        Some(Arc::clone(&raw_close_count)),
    ));
    let pool = DruidPool::builder()
        .name("prepared-getter-failure")
        .db_type_name("mysql")
        .driver_name("cleanup")
        .factory(factory.clone())
        .exception_sorter(Arc::new(CleanupFatalSorter(Some(Arc::clone(
            &sorter_calls,
        )))))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement =
        Connection::prepare_statement(&mut connection, "SELECT 1", StatementOptions::default())
            .await
            .unwrap();
    let error = match failure {
        CleanupFailure::GetResultSet => {
            let _rows = statement.execute_prepared_query().await.unwrap();
            match statement.get_result_set().await {
                Err(error) => error,
                Ok(_) => {
                    panic!("result getter must return the driver error after successful query")
                }
            }
        }
        CleanupFailure::GetUpdateCount => {
            statement.execute_prepared_update().await.unwrap();
            statement.get_update_count().unwrap_err()
        }
        CleanupFailure::GetGeneratedKeys => {
            statement.execute_prepared_update().await.unwrap();
            match statement.generated_keys().await {
                Err(error) => error,
                Ok(_) => panic!("generated keys getter must return the driver error"),
            }
        }
        CleanupFailure::GetMoreResults => {
            let _rows = statement.execute_prepared_query().await.unwrap();
            let result_set = statement.get_result_set().await.unwrap().unwrap();
            let error = statement
                .get_more_results(CurrentResultDisposition::CloseCurrent)
                .await
                .unwrap_err();
            assert!(
                result_set.is_closed().unwrap(),
                "fatal 驱动错误会立即 discard 连接并级联关闭旧 ResultSet wrapper"
            );
            error
        }
        _ => unreachable!("cleanup variants use dedicated tests"),
    };
    assert!(matches!(error, RdbcError::SqlException(_)));
    assert!(Connection::is_closed(&connection).unwrap());
    assert_eq!(sorter_calls.load(Ordering::Acquire), 1);
    assert!(!connection.is_recycled());
    drop(statement);
    connection.close().await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    // 显式关闭等待原连接完成；shutdown 还必须等待实际补建连接关闭。
    pool.close().await;
    assert_eq!(raw_close_count.load(Ordering::Acquire), 1);
    factory.assert_all_connections_closed_once(pool.state().create_count);
}

/// 完整迁移 BASE48 setter 重载；以真实驱动和 Filter 快照观察公开标准合同。
async fn task6_prepared_setter_overloads_preserve_exact_descriptor_identity() {
    use druid::core::FilterChain;
    use druid_rdbc::resource::RdbcReader;
    use druid_rdbc::statement::PreparedTypeNameArgument;
    use druid_rdbc::types::{RdbcCalendar, RdbcCharacterLength, RdbcObject};
    use prepared_descriptor_recorder::PreparedDescriptorRecorder;

    // BASE 这个用例使用记录型 driver，只验证 descriptor 绑定，不规定真实 driver 读取策略。
    let binding_probe = TrackingState::configured();
    let factories: Vec<Arc<dyn ConnectionFactory>> =
        vec![Arc::new(TrackingFactory::new(binding_probe.clone()))];
    for factory in factories {
        let recorder = Arc::new(PreparedDescriptorRecorder::default());
        recorder.reject_execution.store(true, Ordering::Release);
        let mut chain = FilterChain::new();
        chain.add_before(recorder.clone());
        let pool = DruidPool::builder()
            .factory(factory)
            .filter_chain(Arc::new(chain))
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let mut statement = connection
            .prepare_statement("SELECT 1", StatementOptions::default())
            .await
            .unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
        let time = NaiveTime::from_hms_nano_opt(11, 12, 13, 456_789_000).unwrap();
        let timestamp = NaiveDateTime::new(date, time);
        let calendar = RdbcCalendar::new("Asia/Shanghai").unwrap();
        let ascii = RdbcInputStream::from_bytes(b"ascii".to_vec());
        let binary = RdbcInputStream::from_bytes(vec![0, 1, 2]);
        let unicode = RdbcInputStream::from_bytes(b"unicode".to_vec());
        let blob_stream = RdbcInputStream::from_bytes(vec![3, 4, 5]);
        let character = RdbcReader::from_string("character");
        let national = RdbcReader::from_string("国家字符");
        let clob_reader = RdbcReader::from_string("clob");
        let n_clob_reader = RdbcReader::from_string("nclob");
        // 每一项对应 BASE 的1-based重载位置；保留省略、指定null与int/long身份。
        let descriptors = vec![
            PreparedInputParameter::null(4),
            PreparedInputParameter::null_with_type_name(2002, None),
            PreparedInputParameter::Boolean(true),
            PreparedInputParameter::Byte(-8),
            PreparedInputParameter::Short(16),
            PreparedInputParameter::Int(32),
            PreparedInputParameter::Long(64),
            PreparedInputParameter::Float(1.25),
            PreparedInputParameter::Double(2.5),
            PreparedInputParameter::BigDecimal(Some(BigDecimal::from_str("123.4500").unwrap())),
            PreparedInputParameter::String(Some("text".to_owned())),
            PreparedInputParameter::NString(Some("国字".to_owned())),
            PreparedInputParameter::Bytes(Some(vec![8, 9])),
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Specified(None),
            },
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
            PreparedInputParameter::object(Some(RdbcObject::String("object".to_owned()))),
            PreparedInputParameter::object_with_sql_type(Some(RdbcObject::Integer(21)), 4),
            PreparedInputParameter::object_with_sql_type_and_scale(
                Some(RdbcObject::BigDecimal(
                    BigDecimal::from_str("22.50").unwrap(),
                )),
                3,
                2,
            ),
            PreparedInputParameter::AsciiStream {
                stream: Some(ascii.clone()),
                length: RdbcStreamLength::Unspecified,
            },
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(-1),
            },
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX),
            },
            PreparedInputParameter::UnicodeStream {
                stream: Some(unicode.clone()),
                length: 7,
            },
            PreparedInputParameter::BinaryStream {
                stream: Some(binary.clone()),
                length: RdbcStreamLength::Unspecified,
            },
            PreparedInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(-2),
            },
            PreparedInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(29),
            },
            PreparedInputParameter::CharacterStream {
                reader: Some(character.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
            PreparedInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(-3),
            },
            PreparedInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(32),
            },
            PreparedInputParameter::NCharacterStream {
                reader: Some(national.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
            PreparedInputParameter::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(34),
            },
            PreparedInputParameter::Ref(None),
            PreparedInputParameter::Blob(None),
            PreparedInputParameter::BlobStream {
                stream: Some(blob_stream.clone()),
                length: RdbcStreamLength::Unspecified,
            },
            PreparedInputParameter::BlobStream {
                stream: None,
                length: RdbcStreamLength::Long(38),
            },
            PreparedInputParameter::Clob(None),
            PreparedInputParameter::ClobReader {
                reader: Some(clob_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
            PreparedInputParameter::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(41),
            },
            PreparedInputParameter::NClob(None),
            PreparedInputParameter::NClobReader {
                reader: Some(n_clob_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
            PreparedInputParameter::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(44),
            },
            PreparedInputParameter::Array(None),
            PreparedInputParameter::Url(None),
            PreparedInputParameter::RowId(None),
            PreparedInputParameter::SqlXml(None),
        ];
        for (offset, descriptor) in descriptors.iter().enumerate() {
            statement
                .set_input_parameter(ParameterIndex::new(offset + 1).unwrap(), descriptor.clone())
                .unwrap();
        }
        let error = statement.execute_prepared().await.unwrap_err();
        assert_eq!(
            error,
            RdbcError::Other("descriptor-execution-rejected".to_owned())
        );
        let snapshot = recorder.snapshots.lock().unwrap()[0].clone();
        assert_eq!(
            binding_probe
                .prepared_input_bindings
                .lock()
                .unwrap()
                .iter()
                .map(|(_, input)| input.clone())
                .collect::<Vec<_>>(),
            descriptors
        );
        assert_eq!(snapshot.len(), 48);
        assert_eq!(snapshot, descriptors, "池化层不得丢失描述符重载身份");
        // 独立于构造器的字面断言保留 BASE 关键身份。
        assert_eq!(
            snapshot[0],
            PreparedInputParameter::Null {
                sql_type: 4,
                type_name: PreparedTypeNameArgument::Unspecified
            }
        );
        assert_eq!(
            snapshot[1],
            PreparedInputParameter::Null {
                sql_type: 2002,
                type_name: PreparedTypeNameArgument::Specified(None)
            }
        );
        assert_eq!(
            snapshot[14],
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(Some(calendar))
            }
        );
        assert!(matches!(
            snapshot[21],
            PreparedInputParameter::Object {
                target_sql_type: Some(3),
                scale_or_length: Some(2),
                ..
            }
        ));
        assert!(matches!(
            snapshot[23],
            PreparedInputParameter::AsciiStream {
                length: RdbcStreamLength::Int(-1),
                ..
            }
        ));
        assert!(matches!(
            snapshot[30],
            PreparedInputParameter::CharacterStream {
                length: RdbcCharacterLength::Int(-3),
                ..
            }
        ));
        assert!(matches!(
            snapshot[43],
            PreparedInputParameter::NClobReader {
                length: RdbcCharacterLength::Long(44),
                ..
            }
        ));
        assert_eq!(ascii.read_to_end().unwrap(), b"ascii");
        assert_eq!(binary.read_to_end().unwrap(), vec![0, 1, 2]);
        assert_eq!(unicode.read_to_end().unwrap(), b"unicode");
        assert_eq!(blob_stream.read_to_end().unwrap(), vec![3, 4, 5]);
        assert_eq!(character.read_to_string().unwrap(), "character");
        assert_eq!(national.read_to_string().unwrap(), "国家字符");
        assert_eq!(clob_reader.read_to_string().unwrap(), "clob");
        assert_eq!(n_clob_reader.read_to_string().unwrap(), "nclob");

        statement.clear_parameters().unwrap();
        assert_eq!(
            statement.execute_prepared().await.unwrap_err(),
            RdbcError::Other("descriptor-execution-rejected".to_owned())
        );
        let cleared = recorder.snapshots.lock().unwrap()[1].clone();
        assert_eq!(cleared.len(), 0);
        assert!(cleared.is_empty());
        // Typed index 在进入 setter 前拒绝0，不留下逻辑槽位。
        let error = ParameterIndex::new(0)
            .and_then(|index| statement.set_input_parameter(index, PreparedInputParameter::Int(1)))
            .unwrap_err();
        assert!(matches!(error, RdbcError::InvalidArgument(_)));
        assert_eq!(
            statement.execute_prepared().await.unwrap_err(),
            RdbcError::Other("descriptor-execution-rejected".to_owned())
        );
        assert_eq!(recorder.snapshots.lock().unwrap()[2].len(), 0);
        recorder.reject_execution.store(false, Ordering::Release);
        let mut rows = statement.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(22)
        );
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn prepared_all_setter_descriptors_survive_pooling_without_resource_reads() {
    task6_prepared_setter_overloads_preserve_exact_descriptor_identity().await;
}

/// 恢复 BASE ResultSet#getStatement 的叶子身份、同一语句、强所有权和关闭级联合同。
/// 旧 Any/Physical/具体驱动解包按单一标准规格改为 canonical Wrapper 接口。
async fn task6_prepared_result_set_keeps_the_same_statement_alive_and_can_close_it() {
    use druid_rdbc::statement::CallableStatement;
    use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut};
    use std::any::TypeId;

    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let mut original = connection
            .prepare_statement("SELECT ?1 AS value", StatementOptions::default())
            .await
            .unwrap();
        original
            .set_parameter(parameter(1, Value::Int(11)))
            .unwrap();
        let mut result_set = original.execute_prepared_query().await.unwrap();
        assert!(!result_set.is_closed().unwrap());
        assert!(!original.is_closed().unwrap());
        let mut identity = result_set
            .get_statement()
            .unwrap()
            .expect("Prepared 结果必须保留创建语句");
        assert!(!identity.is_closed().unwrap());
        assert!(matches!(
            identity
                .unwrap(Some(TypeId::of::<dyn druid_rdbc::wrapper::Wrapper>()))
                .unwrap(),
            StandardUnwrapped::Wrapper(_)
        ));
        // Canonical null target 是参数错误；不再返回旧的 false/None。
        assert!(matches!(
            identity.is_wrapper_for(None),
            Err(RdbcError::InvalidArgument(_))
        ));
        assert!(matches!(
            identity.unwrap(None),
            Err(RdbcError::InvalidArgument(_))
        ));
        assert!(identity
            .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
            .unwrap());
        assert!(matches!(
            identity
                .unwrap(Some(TypeId::of::<dyn Statement>()))
                .unwrap(),
            StandardUnwrapped::Statement(_)
        ));
        assert!(identity
            .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap());
        let StandardUnwrapped::PreparedStatement(prepared_view) = identity
            .unwrap(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap()
        else {
            panic!("返回语句必须保留 Prepared 叶子身份");
        };
        assert_eq!(prepared_view.sql().unwrap(), "SELECT ?1 AS value");
        assert_eq!(
            prepared_view.options().unwrap(),
            original.options().unwrap()
        );
        assert!(!identity
            .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
            .unwrap());
        assert_eq!(
            identity
                .unwrap(Some(TypeId::of::<dyn CallableStatement>()))
                .unwrap_err(),
            RdbcError::feature_not_supported("wrapper_unwrap_target")
        );
        assert!(!identity.is_wrapper_for(Some(TypeId::of::<u128>())).unwrap());
        assert_eq!(
            identity.unwrap(Some(TypeId::of::<u128>())).unwrap_err(),
            RdbcError::feature_not_supported("wrapper_unwrap_target")
        );

        // 同一 SQL 不足以证明同一语句；两个方向的属性变更必须作用于原对象。
        original.set_query_timeout(7).unwrap();
        assert_eq!(identity.query_timeout().unwrap(), 7);
        identity.set_query_timeout(8).unwrap();
        assert_eq!(original.query_timeout().unwrap(), 8);
        let mut statement_object = result_set.get_statement().unwrap().unwrap();
        assert!(statement_object
            .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
            .unwrap());
        assert!(statement_object
            .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap());
        assert!(matches!(
            statement_object
                .unwrap(Some(TypeId::of::<dyn PreparedStatement>()))
                .unwrap(),
            StandardUnwrapped::PreparedStatement(_)
        ));
        assert!(!statement_object
            .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
            .unwrap());
        assert_eq!(statement_object.query_timeout().unwrap(), 8);
        assert!(!statement_object.is_closed().unwrap());

        drop(original);
        assert!(
            !result_set
                .get_statement()
                .unwrap()
                .unwrap()
                .is_closed()
                .unwrap(),
            "结果集必须持有原语句，不能随原句柄 Drop 提前回收"
        );
        assert!(result_set.next().await.unwrap());
        assert_eq!(
            result_set.object(ColumnIndex::new(1).unwrap()).unwrap(),
            druid_rdbc::types::RdbcObject::Scalar(Value::Int(11))
        );
        // 独占标准叶子视图关闭原语句，所有兄弟句柄与现有结果集一起关闭。
        let StandardUnwrappedMut::PreparedStatement(prepared_view) = identity
            .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap()
        else {
            panic!("可变解包必须操作同一个 Prepared 对象");
        };
        prepared_view.close().await.unwrap();
        prepared_view.close().await.unwrap();
        assert!(identity.is_closed().unwrap());
        assert!(statement_object.is_closed().unwrap());
        assert!(result_set
            .get_statement()
            .unwrap()
            .unwrap()
            .is_closed()
            .unwrap());
        assert!(
            result_set.is_closed().unwrap(),
            "getStatement 关闭必须级联结果集"
        );
        statement_object.close().await.unwrap();
        result_set.close().await.unwrap();
        let closed_owner = result_set.get_statement().unwrap().unwrap();
        assert!(closed_owner.is_closed().unwrap());
        assert!(closed_owner
            .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap());
        // 有效连接中的已关闭游标，与归还连接后的旧租约保持两个不同错误边界。
        assert!(matches!(
            result_set.next().await,
            Err(RdbcError::Other(message)) if message == "result set is closed"
        ));
        connection.close().await.unwrap();
        assert!(matches!(
            result_set.next().await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        assert!(matches!(
            result_set.get_statement(),
            Err(RdbcError::ConnectionDiscarded)
        ));
        pool.close().await;
    }
}

#[tokio::test]
async fn prepared_result_set_keeps_original_statement_alive_and_closes_all_views() {
    task6_prepared_result_set_keeps_the_same_statement_alive_and_can_close_it().await;
}

async fn task6_prepared_result_getters_route_driver_failures_through_connection_sorter() {
    for failure in [
        CleanupFailure::GetResultSet,
        CleanupFailure::GetUpdateCount,
        CleanupFailure::GetGeneratedKeys,
        CleanupFailure::GetMoreResults,
    ] {
        task6_assert_prepared_getter_failure_discards_connection(failure).await;
    }
}

#[tokio::test]
async fn prepared_result_getter_driver_error_discards_connection() {
    task6_assert_prepared_getter_failure_discards_connection(CleanupFailure::GetResultSet).await;
}
#[tokio::test]
async fn prepared_update_count_getter_driver_error_discards_connection() {
    task6_assert_prepared_getter_failure_discards_connection(CleanupFailure::GetUpdateCount).await;
}
#[tokio::test]
async fn prepared_generated_keys_getter_driver_error_discards_connection() {
    task6_assert_prepared_getter_failure_discards_connection(CleanupFailure::GetGeneratedKeys)
        .await;
}
#[tokio::test]
async fn prepared_more_results_getter_driver_error_discards_connection() {
    task6_assert_prepared_getter_failure_discards_connection(CleanupFailure::GetMoreResults).await;
}

async fn task6_concurrent_logical_statements_preserve_java_in_use_and_lru_semantics() {
    let (pool, _) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut seed = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .unwrap();
    seed.close().await.unwrap();
    assert!(seed.is_closed().unwrap());
    drop(seed);
    let mut active = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
    for sql in ["SELECT 1", "SELECT 2", "SELECT 3", "SELECT 4"] {
        let mut statement = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .unwrap();
        statement.close().await.unwrap();
    }
    assert_eq!(pool.state().cached_prepared_statement_count, 3);
    assert!(!Connection::is_closed(&connection).unwrap());
    assert!(!active.is_closed().unwrap());
    active.close().await.unwrap();
    assert!(active.is_closed().unwrap());
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_holder_statement_pool_is_lazy_and_debug_does_not_initialize_it() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state)),
        301,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    assert!(!holder.is_pool_prepared_statements());
    assert_eq!(
        holder
            .prepared_statement_stats()
            .cached_prepared_statement_count(),
        0
    );
    let debug = format!("{holder:?}");
    assert!(debug.contains("DruidConnectionHolder"));
    assert!(!holder.is_pool_prepared_statements());
    assert_eq!(
        holder
            .prepared_statement_stats()
            .cached_prepared_statement_count(),
        0
    );
}

async fn task6_mysql_schema_change_clears_existing_statement_cache() {
    let (pool, _) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    drop(statement);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);
    connection.set_schema(Some("tenant")).await.unwrap();
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_prepared_batch_snapshots_parameters_and_executes_against_real_sqlite() {
    let pool = task6_sqlx_prepared_pool(1).await;
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE IF NOT EXISTS batch_case (id INTEGER PRIMARY KEY, value TEXT)",
            vec![],
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement(
            "INSERT INTO batch_case(id, value) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    // BASE 断言：batch 参数在 execute 后被消费（batch_size 归零）。
    for extra in 1_i64..=2 {
        prepared
            .add_parameter_batch(vec![
                parameter(1, Value::Int(extra)),
                parameter(2, Value::String(format!("batch-{extra}"))),
            ])
            .unwrap();
    }
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1, 1]);
    // 复用同一 prepared handle 再执行一批。
    for extra in 3_i64..=3 {
        prepared
            .add_parameter_batch(vec![
                parameter(1, Value::Int(extra)),
                parameter(2, Value::String(format!("batch-{extra}"))),
            ])
            .unwrap();
    }
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1]);
    // BASE 语义：空批次执行返回空结果且不报错（参数批次已被消费）。
    prepared.clear_prepared_batch().unwrap();
    let empty_batch = prepared.execute_prepared_batch().await.unwrap();
    assert!(empty_batch.is_empty());
    assert_eq!(empty_batch.len(), 0);
    assert_eq!(empty_batch, Vec::<i64>::new());
    assert_eq!(
        PreparedStatement::sql(&*prepared),
        Ok("INSERT INTO batch_case(id, value) VALUES (?, ?)")
    );
    prepared.clear_prepared_batch().unwrap();
    prepared.close().await.unwrap();

    // BASE 断言：行值精确匹配（顺序 + 内容）。
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("batch-1".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
        ]
    );
    // BASE 断言：INSERT 主键重复时旧行被覆盖（SQLite UPSERT 语义）。
    connection
        .exec(
            "INSERT OR REPLACE INTO batch_case(id, value) VALUES (1, 'replaced')",
            vec![],
        )
        .await
        .unwrap();
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("replaced".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
        ]
    );
    // clear_parameters 不破坏 handle 后续可用性（BASE 复用同一 handle 语义）。
    let mut prepared2 = connection
        .prepare_statement(
            "INSERT INTO batch_case(id, value) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    prepared2
        .set_parameter(parameter(1, Value::Int(5)))
        .unwrap();
    prepared2
        .set_parameter(parameter(2, Value::String("five".to_string())))
        .unwrap();
    assert_eq!(prepared2.execute_prepared_update().await.unwrap(), 1);
    prepared2.clear_parameters().unwrap();
    assert_eq!(
        PreparedStatement::sql(&*prepared2),
        Ok("INSERT INTO batch_case(id, value) VALUES (?, ?)")
    );
    let rows = connection
        .fetch("SELECT id, value FROM batch_case ORDER BY id", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows.iter()
            .map(|row| row.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![Value::Int(1), Value::String("replaced".to_string())],
            vec![Value::Int(2), Value::String("batch-2".to_string())],
            vec![Value::Int(3), Value::String("batch-3".to_string())],
            vec![Value::Int(5), Value::String("five".to_string())],
        ]
    );
    prepared2.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_prepared_bound_setters_execute_and_batch_against_real_sqlite() {
    // 保持 BASE 的真实 Toasty/SQLite 路径及全部十五类 setter 语义。
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE prepared_bound_item(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bool_value BOOLEAN, byte_value INTEGER, short_value INTEGER,
            int_value INTEGER, long_value INTEGER, float_value REAL,
            double_value REAL, decimal_value DECIMAL, string_value TEXT,
            n_string_value TEXT, bytes_value BLOB, date_value DATE,
            time_value TIME, timestamp_value DATETIME, null_value TEXT
        )",
            vec![],
        )
        .await
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(12, 13, 14, 123_456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let decimal = BigDecimal::from_str("123.4500").unwrap();
    let mut insert = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(
            bool_value, byte_value, short_value, int_value, long_value,
            float_value, double_value, decimal_value, string_value,
            n_string_value, bytes_value, date_value, time_value,
            timestamp_value, null_value
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    for (offset, input) in [
        PreparedInputParameter::Boolean(true),
        PreparedInputParameter::Byte(-8),
        PreparedInputParameter::Short(16),
        PreparedInputParameter::Int(31),
        PreparedInputParameter::Long(i64::MAX),
        PreparedInputParameter::Float(1.25),
        PreparedInputParameter::Double(2.5),
        PreparedInputParameter::BigDecimal(Some(decimal.clone())),
        PreparedInputParameter::String(Some("text".to_owned())),
        PreparedInputParameter::NString(Some("国家字符".to_owned())),
        PreparedInputParameter::Bytes(Some(vec![0, 1, 2])),
        PreparedInputParameter::Date {
            value: Some(date),
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::Time {
            value: Some(time),
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::Timestamp {
            value: Some(timestamp),
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::null(12),
    ]
    .into_iter()
    .enumerate()
    {
        insert
            .set_input_parameter(ParameterIndex::new(offset + 1).unwrap(), input)
            .unwrap();
    }
    insert
        .set_input_parameter(
            ParameterIndex::new(4).unwrap(),
            PreparedInputParameter::Int(32),
        )
        .unwrap();
    assert_eq!(insert.execute_prepared_update().await.unwrap(), 1);
    let rows = connection
        .fetch(
            "SELECT bool_value, byte_value, short_value, int_value, long_value,
            float_value, double_value, decimal_value, string_value,
            n_string_value, bytes_value, date_value, time_value,
            timestamp_value, null_value FROM prepared_bound_item",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].values,
        vec![
            Value::Int(1),
            Value::Int(-8),
            Value::Int(16),
            Value::Int(32),
            Value::Int(i64::MAX),
            Value::Float(1.25),
            Value::Float(2.5),
            Value::Decimal(BigDecimal::from_str("123.45").unwrap()),
            Value::String("text".to_owned()),
            Value::String("国家字符".to_owned()),
            Value::Bytes(vec![0, 1, 2]),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
            Value::Null,
        ]
    );
    // clear 的效果由再次执行的缺参错误证明，替代已删除的槽位计数 API。
    insert.clear_parameters().unwrap();
    assert!(matches!(
        insert.execute_prepared_update().await,
        Err(RdbcError::InvalidArgument(_))
    ));
    insert.close().await.unwrap();
    assert_eq!(insert.is_closed(), Ok(true));

    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(string_value) VALUES (?1)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::String(Some("first".to_owned())),
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::String(Some("second".to_owned())),
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    assert_eq!(batch.execute_prepared_batch().await.unwrap(), [1, 1]);
    batch.close().await.unwrap();
    let values = connection.fetch(
        "SELECT string_value FROM prepared_bound_item WHERE string_value IS NOT NULL ORDER BY id",
        vec![]).await.unwrap();
    assert_eq!(
        values
            .iter()
            .map(|row| row.values[0].clone())
            .collect::<Vec<_>>(),
        vec![
            Value::String("text".to_owned()),
            Value::String("first".to_owned()),
            Value::String("second".to_owned()),
        ]
    );

    let mut query = connection
        .prepare_statement("SELECT ?1", StatementOptions::default())
        .await
        .unwrap();
    query
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::Int(41),
        )
        .unwrap();
    let mut result_set = query.execute_prepared_query().await.unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(41)
    );
    assert!(!result_set.next().await.unwrap());
    result_set.close().await.unwrap();
    query.close().await.unwrap();

    let mut eager_query = connection
        .prepare_statement("SELECT ?1", StatementOptions::default())
        .await
        .unwrap();
    eager_query
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::Long(42),
        )
        .unwrap();
    let mut eager_result_set = eager_query.execute_prepared_query().await.unwrap();
    let mut eager_values = Vec::new();
    while eager_result_set.next().await.unwrap() {
        eager_values.push(druid_rdbc::types::Row::new(vec![eager_result_set
            .value(ColumnIndex::new(1).unwrap())
            .unwrap()]));
    }
    assert_eq!(
        eager_values,
        vec![druid_rdbc::types::Row::new(vec![Value::Int(42)])]
    );
    eager_result_set.close().await.unwrap();
    eager_query.close().await.unwrap();

    let mut generic_query = connection
        .prepare_statement("SELECT ?1", StatementOptions::default())
        .await
        .unwrap();
    generic_query
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::Short(43),
        )
        .unwrap();
    let mut generic_results = generic_query.execute_prepared().await.unwrap();
    assert_eq!(generic_results.len(), 1);
    assert!(matches!(
        generic_results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    let ExecutionResult::ResultSet(mut generic_result_set) = generic_results.remove(0) else {
        panic!("query must return a result set")
    };
    assert!(generic_result_set.next().await.unwrap());
    assert_eq!(
        generic_result_set
            .value(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::Int(43)
    );
    generic_result_set.close().await.unwrap();
    generic_query.close().await.unwrap();

    let mut gap = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(int_value) VALUES (?1)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    gap.set_input_parameter(
        ParameterIndex::new(2).unwrap(),
        PreparedInputParameter::Int(2),
    )
    .unwrap();
    assert!(matches!(
        gap.execute_prepared_update().await.unwrap_err(),
        RdbcError::InvalidArgument(_)
    ));
    gap.close().await.unwrap();

    let stream = RdbcInputStream::from_bytes(b"not-eagerly-read".to_vec());
    let mut native_only = connection
        .prepare_statement(
            "INSERT INTO prepared_bound_item(bytes_value) VALUES (?1)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    native_only
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(stream.clone()),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    assert_eq!(native_only.execute_prepared_update().await.unwrap(), 1);
    assert!(stream.read_to_end().unwrap().is_empty());
    let stored = connection
        .fetch(
            "SELECT bytes_value FROM prepared_bound_item WHERE id = 4",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(
        stored[0].values,
        vec![Value::Bytes(b"not-eagerly-read".to_vec())]
    );
    native_only.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

async fn task6_statement_execution_error_removes_handle_instead_of_reusing_it() {
    let (pool, state) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("SELECT failure", StatementOptions::default())
        .await
        .unwrap();
    state.fail_next("prepared_execute_generic");
    assert!(matches!(
        statement.execute_prepared().await,
        Err(RdbcError::SqlException(_))
    ));
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
    assert!(matches!(
        statement.execute_prepared().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    drop(statement);
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
    let pool_state = pool.state();
    // BASE Java closePreapredStatement 对从未入缓存的失败语句也无条件递减。
    assert_eq!(pool_state.cached_prepared_statement_count, -1);
    assert_eq!(pool_state.cached_prepared_statement_delete_count, 1);
    assert_eq!(pool_state.closed_prepared_statement_count, 1);
    // 不再暴露私有 holder；同 SQL 再次 prepare 必须创建新 raw，证明失败句柄未入池。
    let mut replacement = connection
        .prepare_statement("SELECT failure", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        state
            .event_snapshot()
            .iter()
            .filter(|event| event.as_str() == "connection.prepare:SELECT failure")
            .count(),
        2
    );
    let results = replacement.execute_prepared().await.unwrap();
    assert!(!results.is_empty());
    drop(results);
    replacement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn prepared_execution_error_excludes_raw_handle_and_preserves_java_delete_statistics() {
    task6_statement_execution_error_removes_handle_instead_of_reusing_it().await;
}

async fn task6_prepared_close_restores_properties_in_java_order_and_stops_on_first_error() {
    let probe = Arc::new(PropertyProbe {
        events: Mutex::new(Vec::new()),
        fail_restore_once: AtomicBool::new(true),
        fail_capture_once: AtomicBool::new(false),
        fail_close_once: AtomicBool::new(false),
        prepare_count: AtomicUsize::new(0),
    });
    let pool = DruidPool::builder()
        .factory(Arc::new(PropertyFactory(Arc::clone(&probe))))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        probe.events(),
        [
            "get_max_field_size",
            "get_max_rows",
            "get_query_timeout",
            "get_fetch_direction",
            "get_fetch_size"
        ]
    );
    statement.set_max_field_size(21).unwrap();
    statement.set_max_rows(22).unwrap();
    statement.set_query_timeout(23).unwrap();
    statement
        .set_fetch_direction(FetchDirection::Forward)
        .unwrap();
    statement.set_fetch_size(24).unwrap();

    let error = statement
        .close()
        .await
        .expect_err("Java close 必须在首个属性恢复错误处停止");
    assert_eq!(
        error,
        RdbcError::DriverError("restore max rows failed".to_owned())
    );
    assert!(!statement.is_closed().unwrap());
    assert_eq!(
        &probe.events()[10..],
        ["set_max_field_size:11", "set_max_rows:12"]
    );

    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
    assert_eq!(
        &probe.events()[12..],
        [
            "set_max_rows:12",
            "set_query_timeout:13",
            "set_fetch_direction:Reverse",
            "set_fetch_size:14",
            "clear_parameters",
            "clear_batch",
            "close"
        ]
    );
    assert_eq!(probe.prepare_count.load(Ordering::Acquire), 1);
    let mut replacement = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(probe.prepare_count.load(Ordering::Acquire), 2);
    replacement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_property_capture_failure_closes_registered_raw_and_keeps_connection_usable() {
    for cached in [false, true] {
        let probe = Arc::new(PropertyProbe {
            events: Mutex::new(Vec::new()),
            fail_restore_once: AtomicBool::new(false),
            fail_capture_once: AtomicBool::new(!cached),
            fail_close_once: AtomicBool::new(false),
            prepare_count: AtomicUsize::new(0),
        });
        let pool = DruidPool::builder()
            .factory(Arc::new(PropertyFactory(Arc::clone(&probe))))
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        if cached {
            let mut seed = connection
                .prepare_statement("select 1", StatementOptions::default())
                .await
                .unwrap();
            seed.close().await.unwrap();
            probe.events.lock().unwrap().clear();
            probe.fail_capture_once.store(true, Ordering::Release);
        }
        let failed = connection
            .prepare_statement("select 1", StatementOptions::default())
            .await;
        assert!(
            matches!(failed, Err(RdbcError::DriverError(ref message)) if message == "capture max rows failed")
        );
        assert_eq!(
            probe.events(),
            ["get_max_field_size", "get_max_rows", "close"]
        );
        assert_eq!(pool.state().closed_prepared_statement_count, 1);
        assert_eq!(
            pool.state().cached_prepared_statement_delete_count,
            u64::from(cached)
        );
        assert_eq!(
            pool.state().cached_prepared_statement_hit_count,
            u64::from(cached)
        );
        assert_eq!(pool.state().cached_prepared_statement_count, 0);
        let mut replacement = connection
            .prepare_statement("select 1", StatementOptions::default())
            .await
            .unwrap();
        assert_eq!(probe.prepare_count.load(Ordering::Acquire), 2);
        replacement.close().await.unwrap();
        connection.close().await.unwrap();
        assert_eq!(pool.state().active_count, 0);
        pool.close().await;
    }
}

#[tokio::test]
async fn prepared_property_capture_cleanup_retry_counts_only_successful_raw_close() {
    let probe = Arc::new(PropertyProbe {
        events: Mutex::new(Vec::new()),
        fail_restore_once: AtomicBool::new(false),
        fail_capture_once: AtomicBool::new(true),
        fail_close_once: AtomicBool::new(true),
        prepare_count: AtomicUsize::new(0),
    });
    let pool = DruidPool::builder()
        .factory(Arc::new(PropertyFactory(Arc::clone(&probe))))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let failed = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await;
    assert!(
        matches!(failed, Err(RdbcError::DriverError(ref message)) if message == "capture max rows failed")
    );
    assert_eq!(pool.state().closed_prepared_statement_count, 0);
    assert_eq!(
        probe
            .events()
            .iter()
            .filter(|event| *event == "close")
            .count(),
        1
    );
    connection.close().await.unwrap();
    assert_eq!(pool.state().closed_prepared_statement_count, 1);
    assert_eq!(
        probe
            .events()
            .iter()
            .filter(|event| *event == "close")
            .count(),
        2
    );
    connection.close().await.unwrap();
    assert_eq!(pool.state().closed_prepared_statement_count, 1);
    pool.close().await;
}

async fn task6_prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics() {
    use druid::core::{ExecOperation, FilterChain};
    use druid_rdbc::statement::{CurrentResultDisposition, GeneratedKeys};
    use druid_rdbc::wrapper::StandardUnwrappedMut;
    use prepared_execute_recorder::PreparedExecuteRecorder;
    use std::any::TypeId;

    let recorder = Arc::new(PreparedExecuteRecorder::default());
    let mut chain = FilterChain::new();
    chain.add_before(recorder.clone());
    chain.add_after(recorder.clone());
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.exec("CREATE TABLE prepared_execute_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)", Vec::new()).await.unwrap();
    recorder.events.lock().unwrap().clear();
    let sql = "SELECT ?1 AS value";
    let mut query = connection
        .prepare_statement(sql, StatementOptions::default())
        .await
        .unwrap();
    query.set_parameter(parameter(1, Value::Int(7))).unwrap();
    let mut results = query.execute_prepared().await.unwrap();
    assert!(matches!(
        results.as_slice(),
        [ExecutionResult::ResultSet(_)]
    ));
    assert_eq!(query.get_update_count().unwrap(), None);
    let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
        panic!("SELECT 返回游标");
    };
    let mut owner = rows.get_statement().unwrap().expect("查询创建语句");
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(owner
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(!owner.is_closed().unwrap());
    assert_eq!(owner.get_update_count().unwrap(), None);
    let StandardUnwrappedMut::PreparedStatement(leaf) = owner
        .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap()
    else {
        panic!("保留 Prepared 叶子");
    };
    assert_eq!(leaf.sql().unwrap(), sql);
    // 由返回句柄修改绑定；原入口下一次执行的 Filter 参数必须同步变为8。
    leaf.set_parameter(parameter(1, Value::Int(8))).unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(7)
    );
    assert!(!rows.next().await.unwrap());
    assert!(!query
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert!(rows.is_closed().unwrap());
    // 关闭游标仍保留创建语句身份；more-results 不关闭创建语句本身。
    let closed_cursor_owner = rows.get_statement().unwrap().unwrap();
    assert!(!closed_cursor_owner.is_closed().unwrap());
    assert!(closed_cursor_owner
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert_eq!(
        *recorder.events.lock().unwrap(),
        vec![
            (
                sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                true,
                None
            ),
            (
                sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(7)],
                false,
                Some(1)
            ),
        ]
    );
    recorder.events.lock().unwrap().clear();
    let mut results = query.execute_prepared().await.unwrap();
    assert!(matches!(
        results.as_slice(),
        [ExecutionResult::ResultSet(_)]
    ));
    let ExecutionResult::ResultSet(mut invalid_rows) = results.remove(0) else {
        panic!("SELECT 返回游标");
    };
    assert!(matches!(
        query.get_more_results_current(999).await,
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(!invalid_rows.is_closed().unwrap());
    assert!(invalid_rows.next().await.unwrap());
    assert_eq!(
        invalid_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(8)
    );
    assert_eq!(
        *recorder.events.lock().unwrap(),
        vec![
            (
                sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(8)],
                true,
                None
            ),
            (
                sql.to_owned(),
                ExecOperation::Execute,
                vec![Value::Int(8)],
                false,
                Some(1)
            ),
        ]
    );
    query.close().await.unwrap();
    assert!(query.is_closed().unwrap());
    assert!(owner.is_closed().unwrap());
    assert!(invalid_rows.is_closed().unwrap());
    owner.close().await.unwrap();

    let insert_sql = "INSERT INTO prepared_execute_item(value) VALUES (?1)";
    for (offset, (label, request)) in [
        ("plain", GeneratedKeys::None),
        ("auto", GeneratedKeys::ReturnGeneratedKeys),
        (
            "indexed",
            GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        ),
        ("named", GeneratedKeys::ColumnNames(vec!["id".to_owned()])),
    ]
    .into_iter()
    .enumerate()
    {
        let mut insert = connection
            .prepare_statement_with_generated_keys(insert_sql, StatementOptions::default(), request)
            .await
            .unwrap();
        insert
            .set_parameter(parameter(1, Value::String(label.to_owned())))
            .unwrap();
        let results = insert.execute_prepared().await.unwrap();
        assert!(matches!(
            results.as_slice(),
            [ExecutionResult::UpdateCount(1)]
        ));
        assert_eq!(insert.get_update_count().unwrap(), Some(1));
        let mut keys = insert.generated_keys().await.unwrap();
        let mut key_owner = keys.get_statement().unwrap().expect("生成键创建语句");
        assert!(key_owner
            .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap());
        assert!(!insert.is_closed().unwrap());
        let StandardUnwrappedMut::PreparedStatement(leaf) = key_owner
            .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap()
        else {
            panic!("生成键保留 Prepared 叶子");
        };
        assert_eq!(leaf.sql().unwrap(), insert_sql);
        assert_eq!(keys.meta_data().unwrap().column_count(), 1);
        assert!(keys.next().await.unwrap());
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(i64::try_from(offset + 1).unwrap())
        );
        assert!(!keys.next().await.unwrap());
        // 结果关闭不关闭创建语句；返回句柄关闭与原句柄关闭共享同一资格。
        keys.close().await.unwrap();
        assert!(keys.is_closed().unwrap());
        assert!(!insert.is_closed().unwrap());
        key_owner.close().await.unwrap();
        assert!(insert.is_closed().unwrap());
        assert!(key_owner.is_closed().unwrap());
        insert.close().await.unwrap();
    }
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut table_rows = statement
        .execute_query("SELECT id, value FROM prepared_execute_item ORDER BY id")
        .await
        .unwrap();
    let mut stored = Vec::new();
    while table_rows.next().await.unwrap() {
        stored.push(vec![
            table_rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            table_rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
        ]);
    }
    table_rows.close().await.unwrap();
    statement.close().await.unwrap();
    assert_eq!(
        stored,
        vec![
            vec![Value::Int(1), Value::String("plain".to_owned())],
            vec![Value::Int(2), Value::String("auto".to_owned())],
            vec![Value::Int(3), Value::String("indexed".to_owned())],
            vec![Value::Int(4), Value::String("named".to_owned())],
        ]
    );
    connection.close().await.unwrap();
    pool.close().await;
}

/// 完整保留 BASE prepared_generic_execute_preserves_ordered_multi_results 的混合结果语义。
async fn task6_prepared_generic_execute_preserves_ordered_multi_results() {
    let (pool, _) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("MULTI", StatementOptions::default())
        .await
        .unwrap();
    let results = statement.execute_prepared().await.unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    assert_eq!(statement.get_update_count().unwrap(), None);
    let mut first = statement.get_result_set().await.unwrap().unwrap();
    assert!(first.next().await.unwrap());
    assert_eq!(
        first.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(10)
    );

    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(first.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), Some(2));
    let mut generated_keys = statement.generated_keys().await.unwrap();
    assert!(generated_keys.next().await.unwrap());
    assert_eq!(
        generated_keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(99)
    );
    generated_keys.close().await.unwrap();

    assert!(statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    let mut third = statement.get_result_set().await.unwrap().unwrap();
    assert!(third.next().await.unwrap());
    assert_eq!(
        third.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(20)
    );
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(third.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

/// 保留 BASE 全部标量类型、nullable 与 native-only 错误合同。
fn task6_prepared_input_parameter_converts_every_scalar_family_without_losing_type() {
    use druid_rdbc::resource::RdbcReader;
    use druid_rdbc::types::{RdbcObject, RdbcUrl};
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(1, 2, 3, 4).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let decimal = BigDecimal::from_str("123.4500").unwrap();
    let url = RdbcUrl::new("https://example.com/path");
    let scalar_cases = vec![
        (PreparedInputParameter::null(4), Value::Null),
        (
            PreparedInputParameter::null_with_type_name(2002, None),
            Value::Null,
        ),
        (PreparedInputParameter::Boolean(true), Value::Bool(true)),
        (PreparedInputParameter::Byte(-8), Value::Int(-8)),
        (PreparedInputParameter::Short(16), Value::Int(16)),
        (PreparedInputParameter::Int(32), Value::Int(32)),
        (PreparedInputParameter::Long(64), Value::Int(64)),
        (PreparedInputParameter::Float(1.25), Value::Float(1.25)),
        (PreparedInputParameter::Double(2.5), Value::Float(2.5)),
        (
            PreparedInputParameter::BigDecimal(Some(decimal.clone())),
            Value::Decimal(decimal.clone()),
        ),
        (PreparedInputParameter::BigDecimal(None), Value::Null),
        (
            PreparedInputParameter::String(Some("text".to_string())),
            Value::String("text".to_string()),
        ),
        (PreparedInputParameter::String(None), Value::Null),
        (
            PreparedInputParameter::NString(Some("国家字符".to_string())),
            Value::String("国家字符".to_string()),
        ),
        (PreparedInputParameter::NString(None), Value::Null),
        (
            PreparedInputParameter::Bytes(Some(vec![0, 1, 2])),
            Value::Bytes(vec![0, 1, 2]),
        ),
        (PreparedInputParameter::Bytes(None), Value::Null),
        (
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Date(date),
        ),
        (
            PreparedInputParameter::Date {
                value: None,
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Time(time),
        ),
        (
            PreparedInputParameter::Time {
                value: None,
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Timestamp(timestamp),
        ),
        (
            PreparedInputParameter::Timestamp {
                value: None,
                calendar: RdbcCalendarArgument::Unspecified,
            },
            Value::Null,
        ),
        (PreparedInputParameter::object(None), Value::Null),
        (
            PreparedInputParameter::object(Some(RdbcObject::Scalar(Value::Int(1)))),
            Value::Int(1),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::String("s".to_string()))),
            Value::String("s".to_string()),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::NString("n".to_string()))),
            Value::String("n".to_string()),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Boolean(true))),
            Value::Bool(true),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Byte(-1))),
            Value::Int(-1),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Short(2))),
            Value::Int(2),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Integer(3))),
            Value::Int(3),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Long(4))),
            Value::Int(4),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Float(5.5))),
            Value::Float(5.5),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Double(6.5))),
            Value::Float(6.5),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Bytes(vec![7]))),
            Value::Bytes(vec![7]),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::BigDecimal(decimal.clone()))),
            Value::Decimal(decimal),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Date(date))),
            Value::Date(date),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Time(time))),
            Value::Time(time),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Timestamp(timestamp))),
            Value::Timestamp(timestamp),
        ),
        (
            PreparedInputParameter::object(Some(RdbcObject::Url(url.clone()))),
            Value::String("https://example.com/path".to_string()),
        ),
        (
            PreparedInputParameter::Url(Some(url)),
            Value::String("https://example.com/path".to_string()),
        ),
        (PreparedInputParameter::Url(None), Value::Null),
        (PreparedInputParameter::Ref(None), Value::Null),
        (PreparedInputParameter::Array(None), Value::Null),
        (PreparedInputParameter::Blob(None), Value::Null),
        (PreparedInputParameter::Clob(None), Value::Null),
        (PreparedInputParameter::NClob(None), Value::Null),
        (PreparedInputParameter::RowId(None), Value::Null),
        (PreparedInputParameter::SqlXml(None), Value::Null),
    ];
    for (parameter, expected) in scalar_cases {
        assert_eq!(parameter.scalar_value().unwrap(), expected);
    }

    let typed = PreparedInputParameter::object_with_sql_type(Some(RdbcObject::Integer(8)), 4);
    assert_eq!(typed.scalar_value().unwrap(), Value::Int(8));
    let scaled = PreparedInputParameter::object_with_sql_type_and_scale(
        Some(RdbcObject::BigDecimal(BigDecimal::from(9))),
        3,
        2,
    );
    assert_eq!(
        scaled.scalar_value().unwrap(),
        Value::Decimal(BigDecimal::from(9))
    );

    let reader = RdbcReader::from_string("native-only");
    let object_error = PreparedInputParameter::object(Some(RdbcObject::CharacterStream(reader)))
        .scalar_value()
        .unwrap_err();
    assert_eq!(
        object_error,
        RdbcError::UnsupportedOperation {
            operation: "prepared_object_requires_native_adapter"
        }
    );
    let stream_error = PreparedInputParameter::AsciiStream {
        stream: None,
        length: RdbcStreamLength::Unspecified,
    }
    .scalar_value()
    .unwrap_err();
    assert_eq!(
        stream_error,
        RdbcError::UnsupportedOperation {
            operation: "prepared_parameter_requires_native_adapter"
        }
    );
}

/// 完整迁移跨租约逻辑句柄隔离与物理缓存复用。对应 Java: DruidPooledPreparedStatement 生命周期。
async fn task6_prepared_statement_cannot_cross_connection_lease_boundary() {
    let (pool, probe) = task6_prepared_pool(3).await;
    let mut first_connection = pool.get().await.unwrap();
    let mut seed = first_connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    seed.close().await.unwrap();
    let mut leaked = first_connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
    first_connection.close().await.unwrap();

    let mut second_connection = pool.get().await.unwrap();
    // 标准语句拥有创建租约，不再接收另一个连接参数；归还后仍必须拒绝执行。
    assert!(leaked.execute_prepared_update().await.is_err());
    let mut replacement = second_connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    let mut rows = replacement.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(22)
    );
    rows.close().await.unwrap();
    replacement.close().await.unwrap();
    // 用底层 prepare 事件证明物理缓存安全复用，而不只检查池的统计标签。
    assert_eq!(
        probe
            .event_snapshot()
            .iter()
            .filter(|event| event.as_str() == "connection.prepare:select 1")
            .count(),
        1
    );
    drop(leaked);
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_delete_count, 0);
    assert_eq!(state.closed_prepared_statement_count, 0);
    assert_eq!(probe.prepared_close_count.load(Ordering::Acquire), 0);
    second_connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(probe.prepared_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn prepared_expired_lease_rejects_old_handle_without_destroying_reusable_cache() {
    task6_prepared_statement_cannot_cross_connection_lease_boundary().await;
}

/// 完整迁移标准视图身份、底层缓存复用和 Java 缓存统计。对应 Java: DruidPooledPreparedStatement。
async fn task6_prepared_statement_reuses_physical_handle_and_updates_java_stats() {
    use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
    use std::any::TypeId;

    let (pool, probe) = task6_prepared_pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let mut first = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        first.is_wrapper_for(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(matches!(
        first.unwrap(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(first
        .is_wrapper_for(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap());
    assert!(matches!(
        first.unwrap(Some(TypeId::of::<dyn Wrapper>())).unwrap(),
        StandardUnwrapped::Wrapper(_)
    ));
    assert!(first
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(matches!(
        first.unwrap(Some(TypeId::of::<dyn Statement>())).unwrap(),
        StandardUnwrapped::Statement(_)
    ));
    assert!(first
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    let StandardUnwrapped::PreparedStatement(view) = first
        .unwrap(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap()
    else {
        panic!("必须返回创建语句的标准 Prepared 视图");
    };
    // trait object 的 vtable 可因转换路径不同而不同；对象身份比较数据地址。
    assert!(std::ptr::addr_eq(first.as_ref(), view));
    assert_eq!(view.sql().unwrap(), "select 1");
    assert!(!first.is_wrapper_for(Some(TypeId::of::<u128>())).unwrap());
    assert!(first.unwrap(Some(TypeId::of::<u128>())).is_err());
    assert!(!first.is_closed().unwrap());
    let mut rows = first.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(22)
    );
    assert!(!rows.next().await.unwrap());
    rows.close().await.unwrap();
    first.close().await.unwrap();
    assert!(first.is_closed().unwrap());
    assert!(first
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(matches!(
        first
            .unwrap(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap(),
        StandardUnwrapped::PreparedStatement(_)
    ));
    assert!(matches!(
        first
            .unwrap_mut(Some(TypeId::of::<dyn PreparedStatement>()))
            .unwrap(),
        StandardUnwrappedMut::PreparedStatement(_)
    ));
    assert_eq!(probe.prepared_close_count.load(Ordering::Acquire), 0);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);
    drop(first);

    let mut second = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        probe
            .event_snapshot()
            .iter()
            .filter(|event| event.as_str() == "connection.prepare:select 1")
            .count(),
        1
    );
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
    assert_eq!(second.execute_prepared_update().await.unwrap(), 1);
    second.close().await.unwrap();
    drop(second);
    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_access_count, 2);
    connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(probe.prepared_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn prepared_reuse_preserves_standard_identity_and_java_cache_statistics() {
    task6_prepared_statement_reuses_physical_handle_and_updates_java_stats().await;
}

/// 恢复真实 SQLite 资源 setter 的读取时机和完整执行合同。对应 Java: PreparedStatement#setBinaryStream。
async fn task6_prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite() {
    use druid::core::FilterChain;
    use druid_rdbc::resource::RdbcReader;
    use druid_rdbc::types::{RdbcCharacterLength, RdbcRowId, RdbcUrl};
    use prepared_descriptor_recorder::PreparedDescriptorRecorder;

    let recorder = Arc::new(PreparedDescriptorRecorder::default());
    let mut chain = FilterChain::new();
    chain.add_before(recorder.clone());
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(chain))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.exec("CREATE TABLE prepared_resource_item(ascii_value TEXT, binary_value BLOB, character_value TEXT, n_character_value TEXT, blob_stream_value BLOB, clob_reader_value TEXT, nclob_reader_value TEXT, url_value TEXT, row_id_value BLOB, null_blob_value BLOB)", vec![]).await.unwrap();
    let short = RdbcInputStream::from_bytes(vec![1, 2]);
    let mut invalid = connection
        .prepare_statement("SELECT ?1", StatementOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        invalid.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(short.clone()),
                length: RdbcStreamLength::Int(3)
            }
        ),
        Err(RdbcError::DriverError(_))
    ));
    assert!(short.read_to_end().unwrap().is_empty());
    assert!(matches!(
        invalid.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(vec![1])),
                length: RdbcStreamLength::Int(-1)
            }
        ),
        Err(RdbcError::InvalidArgument(_))
    ));
    invalid.close().await.unwrap();

    let ascii = RdbcInputStream::from_bytes(b"abcdef");
    let binary = RdbcInputStream::from_bytes(vec![0, 1, 2, 3]);
    let character = RdbcReader::from_string("hello");
    let national = RdbcReader::from_string("国家字符");
    let blob = RdbcInputStream::from_bytes(vec![9, 8, 7]);
    let clob = RdbcReader::from_string("clob-tail");
    let nclob = RdbcReader::from_string("国字大对象");
    let mut statement = connection
        .prepare_statement(
            "INSERT INTO prepared_resource_item VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    let inputs = vec![
        PreparedInputParameter::AsciiStream {
            stream: Some(ascii.clone()),
            length: RdbcStreamLength::Int(3),
        },
        PreparedInputParameter::BinaryStream {
            stream: Some(binary.clone()),
            length: RdbcStreamLength::Long(3),
        },
        PreparedInputParameter::CharacterStream {
            reader: Some(character.clone()),
            length: RdbcCharacterLength::Int(3),
        },
        PreparedInputParameter::NCharacterStream {
            reader: Some(national.clone()),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::BlobStream {
            stream: Some(blob.clone()),
            length: RdbcStreamLength::Long(2),
        },
        PreparedInputParameter::ClobReader {
            reader: Some(clob.clone()),
            length: RdbcCharacterLength::Long(4),
        },
        PreparedInputParameter::NClobReader {
            reader: Some(nclob.clone()),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::Url(Some(RdbcUrl::new("https://example.com/路径"))),
        PreparedInputParameter::RowId(Some(RdbcRowId::new(vec![4, 5, 6]))),
        PreparedInputParameter::Blob(None),
    ];
    for (offset, input) in inputs.iter().enumerate() {
        statement
            .set_input_parameter(ParameterIndex::new(offset + 1).unwrap(), input.clone())
            .unwrap();
    }
    // BASE 必须在 setter 返回前推进同步资源；调用者读取余量不能影响已经绑定的值。
    assert_eq!(ascii.read_to_end().unwrap(), b"def");
    assert_eq!(binary.read_to_end().unwrap(), vec![3]);
    assert_eq!(character.read_to_string().unwrap(), "lo");
    assert!(national.read_to_string().unwrap().is_empty());
    assert_eq!(blob.read_to_end().unwrap(), vec![7]);
    assert_eq!(clob.read_to_string().unwrap(), "-tail");
    assert!(nclob.read_to_string().unwrap().is_empty());
    assert_eq!(statement.execute_prepared_update().await.unwrap(), 1);
    let rows = connection
        .fetch("SELECT * FROM prepared_resource_item", vec![])
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![Row::new(vec![
            Value::String("abc".to_owned()),
            Value::Bytes(vec![0, 1, 2]),
            Value::String("hel".to_owned()),
            Value::String("国家字符".to_owned()),
            Value::Bytes(vec![9, 8]),
            Value::String("clob".to_owned()),
            Value::String("国字大对象".to_owned()),
            Value::String("https://example.com/路径".to_owned()),
            Value::Bytes(vec![4, 5, 6]),
            Value::Null
        ])]
    );

    let mut query = connection
        .prepare_statement("SELECT ?1, ?2", StatementOptions::default())
        .await
        .unwrap();
    query
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(vec![1, 3, 5])),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    query
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("query-reader")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    let mut qr = query.execute_prepared_query().await.unwrap();
    assert!(qr.next().await.unwrap());
    assert_eq!(
        qr.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Bytes(vec![1, 3, 5])
    );
    assert_eq!(
        qr.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::String("query-reader".to_owned())
    );
    qr.close().await.unwrap();
    query.close().await.unwrap();

    let mut generic = connection
        .prepare_statement("SELECT ?1", StatementOptions::default())
        .await
        .unwrap();
    generic
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BlobStream {
                stream: Some(RdbcInputStream::from_bytes(vec![2, 4, 6])),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    let results = generic.execute_prepared().await.unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    let mut gr = generic.get_result_set().await.unwrap().unwrap();
    assert!(gr.next().await.unwrap());
    assert_eq!(
        gr.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Bytes(vec![2, 4, 6])
    );
    gr.close().await.unwrap();
    generic.close().await.unwrap();

    let stream = RdbcInputStream::from_bytes(vec![10, 11, 12]);
    let reader = RdbcReader::from_string("第二批");
    let mut batch = connection
        .prepare_statement(
            "INSERT INTO prepared_resource_item(binary_value, character_value) VALUES (?1,?2)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(stream.clone()),
                length: RdbcStreamLength::Int(2),
            },
        )
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("first")),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(vec![20, 21])),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    batch
        .set_input_parameter(
            ParameterIndex::new(2).unwrap(),
            PreparedInputParameter::CharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    batch.add_prepared_batch().unwrap();
    assert_eq!(batch.execute_prepared_batch().await.unwrap(), vec![1, 1]);
    assert_eq!(stream.read_to_end().unwrap(), vec![12]);
    assert!(reader.read_to_string().unwrap().is_empty());
    batch.close().await.unwrap();
    assert_eq!(connection.fetch("SELECT binary_value, character_value FROM prepared_resource_item WHERE binary_value IS NOT NULL ORDER BY rowid LIMIT 2 OFFSET 1",vec![]).await.unwrap(),vec![Row::new(vec![Value::Bytes(vec![10,11]),Value::String("first".to_owned())]),Row::new(vec![Value::Bytes(vec![20,21]),Value::String("第二批".to_owned())])]);
    let recorded = recorder.snapshots.lock().unwrap().clone();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded[0].len(), 10);
    assert!(matches!(
        recorded[0][0],
        PreparedInputParameter::AsciiStream {
            length: RdbcStreamLength::Int(3),
            ..
        }
    ));
    assert!(matches!(recorded[0][9], PreparedInputParameter::Blob(None)));
    let batches = recorder.batches.lock().unwrap().clone();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 2);
    assert!(matches!(
        batches[0][0][0],
        PreparedInputParameter::BinaryStream {
            length: RdbcStreamLength::Int(2),
            ..
        }
    ));
    assert!(matches!(
        batches[0][1][1],
        PreparedInputParameter::CharacterStream {
            length: RdbcCharacterLength::Unspecified,
            ..
        }
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn prepared_sqlite_resources_preserve_binding_time_and_complete_execution_contract() {
    task6_prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite().await;
}

/// 恢复通过结果集原语句 close 返回 cleanup 错误且不伪报成功关闭的合同。
async fn task6_result_set_statement_handle_reports_cleanup_failure(failure: CleanupFailure) {
    let raw_closes = Arc::new(AtomicUsize::new(0));
    let classifications = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(CleanupFactory::new(failure, Some(raw_closes.clone())));
    let pool = DruidPool::builder()
        .name("prepared-result-set-handle-cleanup-failure")
        .db_type_name("mysql")
        .factory(factory.clone())
        .exception_sorter(Arc::new(CleanupFatalSorter(Some(classifications.clone()))))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement("select 1", StatementOptions::default())
        .await
        .unwrap();
    let result_set = statement.execute_prepared_query().await.unwrap();
    let mut handle = result_set
        .get_statement()
        .unwrap()
        .expect("结果集必须保留原 Prepared owner");
    let error = handle.close().await.expect_err("底层清理错误必须返回");
    assert!(matches!(error, RdbcError::SqlException(_)));
    assert_eq!(error, task6_cleanup_sql_error());
    assert!(
        !handle.is_closed().unwrap(),
        "失败 close 不能报告完整逻辑关闭"
    );
    assert_eq!(classifications.load(Ordering::Acquire), 1);
    assert!(Connection::is_closed(&connection).unwrap());
    // 未完成逻辑关闭与租约可用性不同；fatal 后仍必须拒绝执行，不能复活连接。
    assert!(matches!(
        handle.execute_query("select 1").await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    drop(result_set);
    drop(statement);
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
    assert_eq!(raw_closes.load(Ordering::Acquire), 1);
    factory.assert_all_connections_closed_once(pool.state().create_count);
    assert!(handle.is_closed().unwrap(), "registry 最终必须完成逻辑关闭");
    handle.close().await.unwrap();
    handle.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    assert_eq!(raw_closes.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn returned_prepared_owner_preserves_clear_batch_close_failure() {
    task6_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearBatch).await;
}

#[tokio::test]
async fn returned_prepared_owner_preserves_clear_parameters_close_failure() {
    task6_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearParameters)
        .await;
}

#[tokio::test]
async fn task6_prepared_statement_canonical_matrix() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("SQLx connection");
    connection
        .exec(
            "CREATE TABLE prepared_case(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("table");
    let cases = task6_prepared_statement_case_ids();
    let mut executed = Vec::with_capacity(cases.len());

    for (offset, case_id) in cases.iter().enumerate() {
        match *case_id {
            "result_set_statement_handle_reports_clear_batch_failure" => {
                task6_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearBatch).await;
                executed.push(*case_id);
                continue;
            }
            "result_set_statement_handle_reports_clear_parameters_failure" => {
                task6_result_set_statement_handle_reports_cleanup_failure(CleanupFailure::ClearParameters).await;
                executed.push(*case_id);
                continue;
            }
            "prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite" => {
                task6_prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_statement_reuses_physical_handle_and_updates_java_stats" => {
                task6_prepared_statement_reuses_physical_handle_and_updates_java_stats().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_statement_cannot_cross_connection_lease_boundary" => {
                task6_prepared_statement_cannot_cross_connection_lease_boundary().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_setter_overloads_preserve_exact_descriptor_identity" => {
                task6_prepared_setter_overloads_preserve_exact_descriptor_identity().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_result_set_keeps_the_same_statement_alive_and_can_close_it" => {
                task6_prepared_result_set_keeps_the_same_statement_alive_and_can_close_it().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_result_getters_route_driver_failures_through_connection_sorter" => {
                task6_prepared_result_getters_route_driver_failures_through_connection_sorter().await;
                executed.push(*case_id);
                continue;
            }

            "prepared_input_parameter_converts_every_scalar_family_without_losing_type" => {
                task6_prepared_input_parameter_converts_every_scalar_family_without_losing_type();
                executed.push(*case_id);
                continue;
            }
            "prepared_generic_execute_preserves_ordered_multi_results" => {
                task6_prepared_generic_execute_preserves_ordered_multi_results().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics" => {
                task6_prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_close_restores_properties_in_java_order_and_stops_on_first_error" => {
                task6_prepared_close_restores_properties_in_java_order_and_stops_on_first_error().await;
                executed.push(*case_id);
                continue;
            }
            "clear_batch_failure_uses_connection_sorter_before_cache_return" => {
                task6_assert_cleanup_failure_discards_connection(CleanupFailure::ClearBatch).await;
                executed.push(*case_id);
                continue;
            }
            "clear_parameters_failure_uses_connection_sorter_before_cache_return" => {
                task6_assert_cleanup_failure_discards_connection(CleanupFailure::ClearParameters)
                    .await;
                executed.push(*case_id);
                continue;
            }
            "concurrent_logical_statements_preserve_java_in_use_and_lru_semantics" => {
                task6_concurrent_logical_statements_preserve_java_in_use_and_lru_semantics().await;
                executed.push(*case_id);
                continue;
            }
            "direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter" => {
                for failure in [
                    CleanupFailure::SetParameter,
                    CleanupFailure::AddBatch,
                    CleanupFailure::ClearParameters,
                    CleanupFailure::ClearBatch,
                ] {
                    task6_assert_direct_prepared_method_failure_discards_connection(failure).await;
                }
                executed.push(*case_id);
                continue;
            }
            "mysql_schema_change_clears_existing_statement_cache" => {
                task6_mysql_schema_change_clears_existing_statement_cache().await;
                executed.push(*case_id);
                continue;
            }
            "pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle" => {
                task6_pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle().await;
                executed.push(*case_id);
                continue;
            }
            "holder_statement_pool_is_lazy_and_debug_does_not_initialize_it" => {
                task6_holder_statement_pool_is_lazy_and_debug_does_not_initialize_it().await;
                executed.push(*case_id);
                continue;
            }
            "statement_execution_error_removes_handle_instead_of_reusing_it" => {
                task6_statement_execution_error_removes_handle_instead_of_reusing_it().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_batch_snapshots_parameters_and_executes_against_real_sqlite" => {
                task6_prepared_batch_snapshots_parameters_and_executes_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            "prepared_bound_setters_execute_and_batch_against_real_sqlite" => {
                task6_prepared_bound_setters_execute_and_batch_against_real_sqlite().await;
                executed.push(*case_id);
                continue;
            }
            _ => {}
        }
        let id = i64::try_from(offset + 1).unwrap();
        let sql = "INSERT INTO prepared_case(id, value) VALUES (?, ?)";
        let mut prepared = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .expect("prepared insert");
        assert_eq!(PreparedStatement::sql(&*prepared), Ok(sql));
        prepared
            .set_parameter(parameter(1, Value::Int(id)))
            .unwrap();
        prepared
            .set_parameter(parameter(2, Value::String((*case_id).to_owned())))
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        prepared.clear_parameters().unwrap();
        prepared.clear_prepared_batch().unwrap();
        prepared.set_query_timeout(1).unwrap();
        assert_eq!(prepared.query_timeout(), Ok(1));

        if case_id.contains("batch") {
            for extra in 1_i64..=2 {
                prepared
                    .add_parameter_batch(vec![
                        parameter(1, Value::Int(1_000 + id * 10 + extra)),
                        parameter(2, Value::String(format!("{case_id}-{extra}"))),
                    ])
                    .unwrap();
            }
            assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1, 1]);
        }
        if case_id.contains("result") || case_id.contains("generic") {
            let mut query = connection
                .prepare_statement(
                    "SELECT value FROM prepared_case WHERE id = ?",
                    StatementOptions::default(),
                )
                .await
                .unwrap();
            query.set_parameter(parameter(1, Value::Int(id))).unwrap();
            let mut rows = query.execute_prepared_query().await.unwrap();
            assert!(rows.next().await.unwrap());
            assert_eq!(
                rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::String((*case_id).to_owned())
            );
            rows.close().await.unwrap();
            query.close().await.unwrap();
        }
        prepared.close().await.unwrap();
        assert_eq!(prepared.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_prepared_statement_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
}
