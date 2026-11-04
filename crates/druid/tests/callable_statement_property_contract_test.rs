//! `java.sql.CallableStatement` 属性与元数据方法在 Callable 句柄上的绑定合同。
//!
//! 对应 Java: `java.sql.CallableStatement` 继承自 `java.sql.PreparedStatement` 与
//! `java.sql.Statement` 的属性/元数据入口：`getMaxRows`、`setMaxRows`、`getFetchSize`、
//! `setFetchSize`、`getQueryTimeout`、`getResultSetType`、`getResultSetConcurrency`、
//! `getResultSetHoldability`、`getParameterMetaData`、`getMetaData`、
//! `addBatch(String)`、`clearBatch`、`executeBatch`。
//!
//! 合同要点：
//! 1. 池化代理返回的 Callable 句柄必须把属性读写与批次调用透传到驱动层，
//!    并保留驱动实际收到的参数。
//! 2. 驱动未实现可选属性入口时，代理必须回落到创建时的 `StatementOptions`
//!    与代理缓存值，不得把回落当成失败。
//! 3. 驱动未实现 metadata/batch 入口时，代理必须原样透出
//!    `RdbcError::UnsupportedOperation`，且 `operation` 名称必须精确匹配标准合同。

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ParameterMetaData, ResultSetMetaData,
    ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement,
    ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use std::sync::{Arc, Mutex, MutexGuard};

/// 驱动层记录到的单次属性/批次调用。
#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordedCall {
    /// `set_max_rows(i64)` 的实际参数。
    SetMaxRows(i64),
    /// `set_fetch_size(i32)` 的实际参数。
    SetFetchSize(i32),
    /// `set_query_timeout(i32)` 的实际参数。
    SetQueryTimeout(i32),
    /// `add_batch(String)` 的实际 SQL。
    AddBatch(String),
    /// `clear_batch()`。
    ClearBatch,
    /// `execute_batch()`。
    ExecuteBatch,
}

/// 记录型 mock 的内部状态快照。
#[derive(Debug, Default)]
struct RecordingInner {
    calls: Vec<RecordedCall>,
    max_rows: i64,
    fetch_size: i32,
    query_timeout: i32,
    batch: Vec<String>,
}

/// 记录型 mock 与驱动之间共享的状态。
#[derive(Debug, Default)]
struct RecordingState {
    inner: Mutex<RecordingInner>,
}

impl RecordingState {
    /// 取得内部状态的可变借用；中毒时恢复而不是 panic。
    fn lock(&self) -> MutexGuard<'_, RecordingInner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 记录一次驱动层收到的调用。
    fn record(&self, call: RecordedCall) {
        self.lock().calls.push(call);
    }

    /// 返回当前全部调用记录的快照。
    fn calls(&self) -> Vec<RecordedCall> {
        self.lock().calls.clone()
    }

    /// 返回驱动当前持有的 max rows。
    fn max_rows(&self) -> i64 {
        self.lock().max_rows
    }

    /// 更新驱动当前持有的 max rows。
    fn set_max_rows(&self, value: i64) {
        self.lock().max_rows = value;
    }

    /// 返回驱动当前持有的 fetch size。
    fn fetch_size(&self) -> i32 {
        self.lock().fetch_size
    }

    /// 更新驱动当前持有的 fetch size。
    fn set_fetch_size(&self, value: i32) {
        self.lock().fetch_size = value;
    }

    /// 返回驱动当前持有的 query timeout。
    fn query_timeout(&self) -> i32 {
        self.lock().query_timeout
    }

    /// 更新驱动当前持有的 query timeout。
    fn set_query_timeout(&self, value: i32) {
        self.lock().query_timeout = value;
    }

    /// 返回驱动当前持有的 batch 条目。
    fn batch(&self) -> Vec<String> {
        self.lock().batch.clone()
    }

    /// 追加一条 batch 条目。
    fn push_batch(&self, sql: String) {
        self.lock().batch.push(sql);
    }

    /// 清空 batch 条目。
    fn clear_batch(&self) {
        self.lock().batch.clear();
    }
}

/// 记录型 Callable：实现全部可选入口，并把驱动收到的调用记录下来。
struct RecordingCallable {
    sql: String,
    state: Arc<RecordingState>,
    options: StatementOptions,
}

druid_rdbc::impl_standard_wrapper!(RecordingCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for RecordingCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn options(&self) -> Result<StatementOptions, RdbcError> {
        Ok(self.options)
    }

    fn max_rows(&self) -> Result<i64, RdbcError> {
        Ok(self.state.max_rows())
    }

    fn set_max_rows(&mut self, max: i64) -> Result<(), RdbcError> {
        self.state.record(RecordedCall::SetMaxRows(max));
        self.state.set_max_rows(max);
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        Ok(self.state.fetch_size())
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.state.record(RecordedCall::SetFetchSize(rows));
        self.state.set_fetch_size(rows);
        Ok(())
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Ok(self.state.query_timeout())
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.state.record(RecordedCall::SetQueryTimeout(seconds));
        self.state.set_query_timeout(seconds);
        Ok(())
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.state.record(RecordedCall::AddBatch(sql.clone()));
        self.state.push_batch(sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.state.record(RecordedCall::ClearBatch);
        self.state.clear_batch();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.state.record(RecordedCall::ExecuteBatch);
        Ok((1..=self.state.batch().len() as i64).collect())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        _index: ParameterIndex,
        _parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Ok(Arc::new(RecordingParameterMetaData { parameter_count: 3 }))
    }

    fn result_set_meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
            ColumnDescriptor::new("ID", "INTEGER", ColumnNullability::NoNulls),
            ColumnDescriptor::new("NAME", "VARCHAR", ColumnNullability::Nullable),
        ])))
    }
}

#[async_trait::async_trait]
impl CallableStatement for RecordingCallable {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }
}

/// 记录型驱动返回的参数元数据快照。
struct RecordingParameterMetaData {
    parameter_count: usize,
}

druid_rdbc::impl_standard_wrapper!(RecordingParameterMetaData => ParameterMetaData);

impl ParameterMetaData for RecordingParameterMetaData {
    fn parameter_count(&self) -> usize {
        self.parameter_count
    }
}

/// 最小 Callable：只实现必选入口，其余沿用标准默认实现（UnsupportedOperation）。
struct MinimalCallable {
    sql: String,
}

druid_rdbc::impl_standard_wrapper!(MinimalCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for MinimalCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for MinimalCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }
}

#[async_trait::async_trait]
impl CallableStatement for MinimalCallable {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }
}

/// 连接工厂选择的驱动实现。
#[derive(Clone)]
enum DriverKind {
    /// 完整记录型驱动。
    Recording(Arc<RecordingState>),
    /// 最小驱动，只实现标准必选入口。
    Minimal,
}

/// 提供记录型/最小型 Callable 的驱动级连接。
struct RecordingConnection {
    kind: DriverKind,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(match &self.kind {
            DriverKind::Recording(state) => Box::new(RecordingCallable {
                sql: sql.to_owned(),
                state: Arc::clone(state),
                options,
            }),
            DriverKind::Minimal => Box::new(MinimalCallable {
                sql: sql.to_owned(),
            }),
        })
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("recording-callable".to_owned())
    }
}

/// 产出记录型连接的连接工厂。
struct RecordingFactory {
    kind: DriverKind,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            kind: self.kind.clone(),
        }))
    }
}

/// 构建使用指定驱动实现的连接池。
async fn recording_pool(kind: DriverKind) -> DruidPool {
    DruidPool::builder()
        .name("callable-statement-property")
        .driver_name("recording-callable")
        .factory(Arc::new(RecordingFactory { kind }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("recording pool")
}

/// 构建完整记录型驱动对应的连接池。
async fn recording_state_pool(state: Arc<RecordingState>) -> DruidPool {
    recording_pool(DriverKind::Recording(state)).await
}

/// 构建最小驱动对应的连接池。
async fn minimal_pool() -> DruidPool {
    recording_pool(DriverKind::Minimal).await
}

/// 断言结果是精确的 `UnsupportedOperation`，且 operation 名称完全一致。
fn assert_unsupported<T>(result: Result<T, RdbcError>, expected: &str) {
    match result {
        Err(RdbcError::UnsupportedOperation { operation }) => assert_eq!(
            operation, expected,
            "UnsupportedOperation 的 operation 必须精确匹配标准合同"
        ),
        Ok(_) => panic!("预期 UnsupportedOperation({expected})，实际返回 Ok"),
        Err(error) => panic!("预期 UnsupportedOperation({expected})，实际返回 {error:?}"),
    }
}

/// `getMaxRows`/`setMaxRows` 在池化 Callable 句柄上必须回读同一个值。
#[tokio::test]
async fn callable_statement_max_rows_property_round_trips_through_pooled_handle() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call("{call max_rows_probe()}", StatementOptions::default())
        .await
        .expect("callable handle");

    assert_eq!(
        callable.max_rows().expect("initial max rows"),
        0,
        "初始 max rows 必须保持 JDBC 默认的无限制 0"
    );

    callable.set_max_rows(25).expect("set max rows");

    assert_eq!(
        callable.max_rows().expect("max rows readback"),
        25,
        "setMaxRows 之后 getMaxRows 必须回读同一个值"
    );
    assert_eq!(state.max_rows(), 25, "驱动层必须持有同一个 max rows 值");
    assert_eq!(
        state.calls(),
        vec![RecordedCall::SetMaxRows(25)],
        "驱动层必须且只能收到一次 set_max_rows(25)"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `getFetchSize`/`setFetchSize` 在池化 Callable 句柄上必须回读同一个值。
#[tokio::test]
async fn callable_statement_fetch_size_property_round_trips_through_pooled_handle() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call("{call fetch_size_probe()}", StatementOptions::default())
        .await
        .expect("callable handle");

    assert_eq!(
        callable.fetch_size().expect("initial fetch size"),
        0,
        "初始 fetch size 必须保持 JDBC 默认值 0"
    );

    callable.set_fetch_size(50).expect("set fetch size");

    assert_eq!(
        callable.fetch_size().expect("fetch size readback"),
        50,
        "setFetchSize 之后 getFetchSize 必须回读同一个值"
    );
    assert_eq!(state.fetch_size(), 50, "驱动层必须持有同一个 fetch size 值");
    assert_eq!(
        state.calls(),
        vec![RecordedCall::SetFetchSize(50)],
        "驱动层必须且只能收到一次 set_fetch_size(50)"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `getQueryTimeout` 必须返回驱动当前持有的超时秒数。
#[tokio::test]
async fn callable_statement_query_timeout_getter_reports_driver_value() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call("{call query_timeout_probe()}", StatementOptions::default())
        .await
        .expect("callable handle");

    assert_eq!(
        callable.query_timeout().expect("initial query timeout"),
        0,
        "初始 query timeout 必须保持 JDBC 默认值 0"
    );

    callable.set_query_timeout(30).expect("set query timeout");

    assert_eq!(
        callable.query_timeout().expect("query timeout readback"),
        30,
        "getQueryTimeout 必须返回驱动当前持有的超时秒数"
    );
    assert_eq!(
        state.query_timeout(),
        30,
        "驱动层必须持有同一个 query timeout 值"
    );
    assert_eq!(
        state.calls(),
        vec![RecordedCall::SetQueryTimeout(30)],
        "驱动层必须且只能收到一次 set_query_timeout(30)"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `getResultSetType`/`getResultSetConcurrency`/`getResultSetHoldability`
/// 必须返回驱动报告的游标属性，而不是固定的默认值。
#[tokio::test]
async fn callable_statement_cursor_properties_report_driver_values() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );
    let callable = connection
        .prepare_call("{call cursor_property_probe()}", options)
        .await
        .expect("callable handle");

    assert_eq!(
        callable.result_set_type().expect("result set type"),
        ResultSetType::ScrollInsensitive,
        "getResultSetType 必须返回驱动报告的游标类型"
    );
    assert_eq!(
        callable
            .result_set_concurrency()
            .expect("result set concurrency"),
        ResultSetConcurrency::Updatable,
        "getResultSetConcurrency 必须返回驱动报告的并发模式"
    );
    assert_eq!(
        callable
            .result_set_holdability()
            .expect("result set holdability"),
        ResultSetHoldability::HoldOverCommit,
        "getResultSetHoldability 必须返回驱动报告的保留语义"
    );
    assert_eq!(
        callable
            .result_set_type()
            .expect("result set type")
            .rdbc_code(),
        1004,
        "游标类型必须保留 JDBC TYPE_SCROLL_INSENSITIVE 整数语义"
    );
    assert_eq!(
        callable
            .result_set_concurrency()
            .expect("result set concurrency")
            .rdbc_code(),
        1008,
        "并发模式必须保留 JDBC CONCUR_UPDATABLE 整数语义"
    );
    assert_eq!(
        callable
            .result_set_holdability()
            .expect("result set holdability")
            .rdbc_code(),
        1,
        "保留语义必须保留 JDBC HOLD_CURSORS_OVER_COMMIT 整数语义"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `getParameterMetaData`/`getMetaData` 必须返回驱动提供的元数据快照。
#[tokio::test]
async fn callable_statement_metadata_getters_return_driver_snapshots() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let callable = connection
        .prepare_call("{call metadata_probe(?)}", StatementOptions::default())
        .await
        .expect("callable handle");

    let parameter_meta = callable.parameter_meta_data().expect("parameter metadata");
    assert_eq!(
        parameter_meta.parameter_count(),
        3,
        "getParameterMetaData 必须返回驱动报告的参数数量"
    );

    let result_meta = callable
        .result_set_meta_data()
        .expect("result set metadata");
    assert_eq!(
        result_meta.column_count(),
        2,
        "getMetaData 必须返回驱动报告的列数量"
    );
    assert_eq!(
        result_meta
            .column_label(ColumnIndex::new(1).expect("1-based column"))
            .expect("first column label"),
        "ID",
        "getMetaData 必须保留驱动报告的列标签"
    );
    assert_eq!(
        result_meta
            .column_label(ColumnIndex::new(2).expect("1-based column"))
            .expect("second column label"),
        "NAME",
        "getMetaData 必须保留驱动报告的第二列标签"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `addBatch(String)`/`clearBatch`/`executeBatch` 必须透传到驱动并保留批次内容。
#[tokio::test]
async fn callable_statement_batch_entry_points_record_driver_calls() {
    let state = Arc::new(RecordingState::default());
    let pool = recording_state_pool(Arc::clone(&state)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call("{call batch_probe()}", StatementOptions::default())
        .await
        .expect("callable handle");

    callable
        .add_batch("{call batch_probe(1)}".to_owned())
        .expect("add first batch entry");
    callable
        .add_batch("{call batch_probe(2)}".to_owned())
        .expect("add second batch entry");

    assert_eq!(
        state.batch(),
        vec![
            "{call batch_probe(1)}".to_owned(),
            "{call batch_probe(2)}".to_owned()
        ],
        "addBatch(String) 必须把 SQL 原样交给驱动层"
    );

    let counts = callable.execute_batch().await.expect("execute batch");
    assert_eq!(
        counts,
        vec![1, 2],
        "executeBatch 必须返回驱动报告的更新计数序列"
    );

    callable.clear_batch().expect("clear batch");
    assert!(
        state.batch().is_empty(),
        "clearBatch 必须清空驱动层持有的批次条目"
    );
    assert_eq!(
        state.calls(),
        vec![
            RecordedCall::AddBatch("{call batch_probe(1)}".to_owned()),
            RecordedCall::AddBatch("{call batch_probe(2)}".to_owned()),
            RecordedCall::ExecuteBatch,
            RecordedCall::ClearBatch,
        ],
        "驱动层必须按调用顺序收到 add_batch/execute_batch/clear_batch"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 驱动未实现可选属性入口时，代理必须回落到创建时的选项与代理缓存值。
#[tokio::test]
async fn callable_statement_minimal_driver_falls_back_to_proxy_cached_properties() {
    let pool = minimal_pool().await;
    let mut connection = pool.get().await.expect("pooled connection");
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );
    let mut callable = connection
        .prepare_call("{call minimal_property_probe()}", options)
        .await
        .expect("callable handle");

    assert_eq!(
        callable
            .result_set_type()
            .expect("result set type fallback"),
        ResultSetType::ScrollInsensitive,
        "驱动不支持时必须回落到创建时的 result set type"
    );
    assert_eq!(
        callable
            .result_set_concurrency()
            .expect("result set concurrency fallback"),
        ResultSetConcurrency::Updatable,
        "驱动不支持时必须回落到创建时的 concurrency"
    );
    assert_eq!(
        callable
            .result_set_holdability()
            .expect("result set holdability fallback"),
        ResultSetHoldability::HoldOverCommit,
        "驱动不支持时必须回落到创建时的 holdability"
    );
    assert_eq!(
        callable.max_rows().expect("max rows fallback"),
        0,
        "驱动不支持时必须回落到代理缓存的 max rows 默认值"
    );
    assert_eq!(
        callable.fetch_size().expect("fetch size fallback"),
        0,
        "驱动不支持时必须回落到代理缓存的 fetch size 默认值"
    );
    assert_eq!(
        callable.query_timeout().expect("query timeout fallback"),
        0,
        "驱动不支持时必须回落到代理缓存的 query timeout 默认值"
    );

    callable
        .set_max_rows(25)
        .expect("set max rows on minimal driver");
    assert_eq!(
        callable.max_rows().expect("max rows after set"),
        25,
        "驱动不支持 setter 时代理仍必须保存并回读自己的缓存值"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 驱动未实现 metadata/batch 入口时，必须透出精确的 UnsupportedOperation 名称。
#[tokio::test]
async fn callable_statement_optional_entry_points_report_exact_unsupported_operations() {
    let pool = minimal_pool().await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call minimal_optional_probe()}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    assert_unsupported(
        callable.parameter_meta_data(),
        "prepared_statement_parameter_meta_data",
    );
    assert_unsupported(
        callable.result_set_meta_data(),
        "prepared_statement_result_set_meta_data",
    );
    assert_unsupported(
        callable.add_batch("{call minimal_optional_probe(1)}".to_owned()),
        "statement_add_batch",
    );
    assert_unsupported(callable.execute_batch().await, "statement_execute_batch");

    connection.close().await.expect("close connection");
    pool.close().await;
}
