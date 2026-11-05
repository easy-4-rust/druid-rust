//! `java.sql.CallableStatement` 剩余可测重载在池化 Callable 句柄上的合同。
//!
//! 对应 Java（均继承自 `java.sql.PreparedStatement` / `java.sql.Statement`）：
//! 1. `addBatch()` —— 把当前参数快照登记为一条 prepared batch。
//! 2. `clearParameters()` —— 清空当前绑定的参数快照。
//! 3. `getGeneratedKeys()` —— 取回最近一次执行产生的自增键结果集。
//! 4. `setDate(int, Date, Calendar)` / `setTime(int, Time, Calendar)` /
//!    `setTimestamp(int, Timestamp, Calendar)` —— 带 Calendar 的日期时间重载。
//!
//! 合同要点：
//! 1. 池化代理必须把 `addBatch()` 透传到驱动层的 prepared batch 入口，
//!    驱动批次计数按调用次数递增。
//! 2. `clearParameters()` 既要透传驱动层，也要清空代理层持有的参数槽位。
//! 3. `getGeneratedKeys()` 返回的游标可逐行遍历，且行数与驱动层一致。
//! 4. Calendar 重载身份必须精确保留为 `Specified(Some(calendar))`，
//!    不得退化成 `Unspecified`。

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, FilterChain, ResultSetFilter,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedInputParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, Row, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 驱动层与池化代理共享的记录状态。
#[derive(Debug, Default)]
struct RecordingState {
    /// 驱动层收到的 prepared batch 条目数。
    prepared_batches: Mutex<usize>,
    /// 驱动层收到的 `clear_parameters` 次数。
    cleared_parameters: Mutex<usize>,
    /// 驱动层收到的类型化索引绑定。
    bindings: Mutex<Vec<(usize, PreparedInputParameter)>>,
}

impl RecordingState {
    /// 记录一次类型化索引绑定。
    fn record_binding(&self, index: usize, parameter: PreparedInputParameter) {
        self.bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((index, parameter));
    }

    /// 返回当前驱动层 prepared batch 条目数。
    fn prepared_batches(&self) -> usize {
        *self
            .prepared_batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 返回当前驱动层 `clear_parameters` 次数。
    fn cleared_parameters(&self) -> usize {
        *self
            .cleared_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 返回当前全部类型化绑定记录的快照。
    fn bindings(&self) -> Vec<(usize, PreparedInputParameter)> {
        self.bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// 记录剩余重载委托的驱动级 Callable mock。
struct RecordingCallable {
    sql: String,
    state: Arc<RecordingState>,
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

    /// 返回驱动层持有的单行自增键结果集。
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(druid_rdbc::extensions::BufferedRowSet::new(vec![
            Row::new(vec![Value::Int(42)]),
        ])))
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.state.record_binding(index.get(), parameter);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        *self
            .state
            .cleared_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        Ok(())
    }

    fn add_prepared_batch(&mut self) -> Result<(), RdbcError> {
        *self
            .state
            .prepared_batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        Ok(())
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Ok(1)
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

/// 提供记录型 Callable 的驱动级连接。
struct RecordingConnection {
    state: Arc<RecordingState>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_call(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(RecordingCallable {
            sql: sql.to_owned(),
            state: Arc::clone(&self.state),
        }))
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
        Ok("recording-callable-remaining".to_owned())
    }
}

/// 产出记录型连接的连接工厂。
struct RecordingFactory {
    state: Arc<RecordingState>,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            state: Arc::clone(&self.state),
        }))
    }
}

/// 采集执行期参数槽位快照的过滤器。
#[derive(Debug, Default)]
struct SlotSnapshotFilter {
    snapshots: Mutex<Vec<Vec<PreparedInputParameter>>>,
}

impl SlotSnapshotFilter {
    /// 返回采集到的全部槽位快照。
    fn snapshots(&self) -> Vec<Vec<PreparedInputParameter>> {
        self.snapshots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for SlotSnapshotFilter {
    fn name(&self) -> &str {
        "callable-remaining-slot-snapshot"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameters) = context.prepared_parameters {
            self.snapshots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(parameters.to_vec());
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for SlotSnapshotFilter {
    fn name(&self) -> &str {
        "callable-remaining-slot-snapshot"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for SlotSnapshotFilter {}

/// 构建 1-based 参数索引。
fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).expect("1-based parameter index")
}

/// 构建带记录过滤器与记录型工厂的连接池。
async fn recording_pool(state: Arc<RecordingState>, filter: Arc<SlotSnapshotFilter>) -> DruidPool {
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::clone(&filter));
    DruidPool::builder()
        .name("callable-remaining-overload")
        .driver_name("recording-callable-remaining")
        .factory(Arc::new(RecordingFactory { state }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .expect("recording pool")
}

/// `addBatch()` 继承自 `PreparedStatement`，代理必须透传到驱动层 prepared batch 入口。
#[tokio::test]
async fn callable_add_batch_increments_driver_prepared_batch_count() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call("{call remaining_add_batch(?)}", StatementOptions::default())
        .await
        .expect("callable handle");

    assert_eq!(
        state.prepared_batches(),
        0,
        "驱动层初始不得持有 prepared batch 条目"
    );

    callable
        .set_input_parameter(index(1), PreparedInputParameter::Int(7))
        .expect("bind index 1");
    callable.add_prepared_batch().expect("addBatch()");

    assert_eq!(
        state.prepared_batches(),
        1,
        "addBatch() 必须让驱动层 prepared batch 计数 +1"
    );

    callable.add_prepared_batch().expect("addBatch() again");

    assert_eq!(
        state.prepared_batches(),
        2,
        "第二次 addBatch() 必须让驱动层 prepared batch 计数再次 +1"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `clearParameters()` 必须透传驱动层，并清空代理层持有的参数槽位。
#[tokio::test]
async fn callable_clear_parameters_empties_bound_parameter_slots() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call remaining_clear_parameters(?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    callable
        .set_input_parameter(index(1), PreparedInputParameter::Int(7))
        .expect("bind index 1");
    callable
        .set_input_parameter(
            index(2),
            PreparedInputParameter::String(Some("payload".to_owned())),
        )
        .expect("bind index 2");
    assert_eq!(
        state.cleared_parameters(),
        0,
        "驱动层初始不得收到 clear_parameters"
    );

    callable.clear_parameters().expect("clearParameters()");

    assert_eq!(
        state.cleared_parameters(),
        1,
        "clearParameters() 必须透传驱动层一次"
    );

    // 代理层参数槽位必须同时被清空：随后执行时 Filter 看到的参数槽位为空。
    callable
        .execute_prepared_update()
        .await
        .expect("execute prepared update");
    let snapshots = filter.snapshots();
    assert_eq!(snapshots.len(), 1, "执行期必须采集一次参数槽位快照");
    assert!(
        snapshots[0].is_empty(),
        "clearParameters() 之后代理层参数槽位必须为空，实际 {:?}",
        snapshots[0]
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// `getGeneratedKeys()` 继承自 `Statement`，必须返回可遍历的驱动层自增键结果集。
#[tokio::test]
async fn callable_get_generated_keys_returns_traversable_driver_result_set() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call remaining_generated_keys()}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    let mut keys = callable.generated_keys().await.expect("getGeneratedKeys()");

    assert_eq!(
        keys.known_row_count(),
        Some(1),
        "自增键结果集必须包含驱动层返回的单行"
    );
    assert!(keys.next().await.expect("next row"), "必须能读到第一行");
    assert_eq!(
        keys.long(ColumnIndex::new(1).expect("1-based column"))
            .expect("generated key value"),
        42,
        "自增键值必须与驱动层一致"
    );
    assert!(
        !keys.next().await.expect("next row after last"),
        "单行结果集遍历后必须结束"
    );

    keys.close().await.expect("close generated keys");
    assert!(keys.is_closed().expect("generated keys closed"));

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 带 Calendar 的日期时间重载必须精确保留 `Specified(Some(calendar))` 身份。
#[tokio::test]
async fn callable_calendar_setters_keep_specified_calendar_identity() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call remaining_calendar(?, ?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    let calendar = RdbcCalendar::new("Asia/Shanghai").expect("valid calendar time zone");
    let date = NaiveDate::from_ymd_opt(2026, 11, 5).expect("valid date");
    let time = NaiveTime::from_hms_opt(9, 30, 15).expect("valid time");
    let timestamp = NaiveDateTime::new(date, time);

    callable
        .set_input_parameter(
            index(1),
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
        )
        .expect("setDate(int, Date, Calendar)");
    callable
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
        )
        .expect("setTime(int, Time, Calendar)");
    callable
        .set_input_parameter(
            index(3),
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
        )
        .expect("setTimestamp(int, Timestamp, Calendar)");

    let recorded = state.bindings();
    assert_eq!(recorded.len(), 3, "每次 Calendar setter 必须产生一条记录");

    let expected_calendar = RdbcCalendarArgument::Specified(Some(calendar.clone()));

    match &recorded[0].1 {
        PreparedInputParameter::Date { value, calendar } => {
            assert_eq!(value, &Some(date), "setDate 必须保留原值");
            assert_eq!(
                calendar, &expected_calendar,
                "setDate(int, Date, Calendar) 必须保留 Calendar 重载身份"
            );
            assert!(
                !matches!(calendar, RdbcCalendarArgument::Unspecified),
                "Calendar 重载不得退化成 Unspecified"
            );
        }
        other => panic!("setDate 必须记录 Date 描述符，实际 {other:?}"),
    }

    match &recorded[1].1 {
        PreparedInputParameter::Time { value, calendar } => {
            assert_eq!(value, &Some(time), "setTime 必须保留原值");
            assert_eq!(
                calendar, &expected_calendar,
                "setTime(int, Time, Calendar) 必须保留 Calendar 重载身份"
            );
            assert!(
                !matches!(calendar, RdbcCalendarArgument::Unspecified),
                "Calendar 重载不得退化成 Unspecified"
            );
        }
        other => panic!("setTime 必须记录 Time 描述符，实际 {other:?}"),
    }

    match &recorded[2].1 {
        PreparedInputParameter::Timestamp { value, calendar } => {
            assert_eq!(value, &Some(timestamp), "setTimestamp 必须保留原值");
            assert_eq!(
                calendar, &expected_calendar,
                "setTimestamp(int, Timestamp, Calendar) 必须保留 Calendar 重载身份"
            );
            assert!(
                !matches!(calendar, RdbcCalendarArgument::Unspecified),
                "Calendar 重载不得退化成 Unspecified"
            );
        }
        other => panic!("setTimestamp 必须记录 Timestamp 描述符，实际 {other:?}"),
    }

    assert_eq!(recorded[0].0, 1, "setDate 必须落在 1-based 索引 1");
    assert_eq!(recorded[1].0, 2, "setTime 必须落在 1-based 索引 2");
    assert_eq!(recorded[2].0, 3, "setTimestamp 必须落在 1-based 索引 3");

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 代理层参数槽位必须原样保留 Calendar 重载描述符，供过滤器与驱动共同消费。
#[tokio::test]
async fn callable_calendar_setters_reach_execution_parameter_slots() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call remaining_calendar_slots(?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    let calendar = RdbcCalendar::new("Asia/Shanghai").expect("valid calendar time zone");
    let date = NaiveDate::from_ymd_opt(2026, 11, 5).expect("valid date");
    let time = NaiveTime::from_hms_opt(9, 30, 15).expect("valid time");

    callable
        .set_input_parameter(
            index(1),
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
        )
        .expect("setDate(int, Date, Calendar)");
    callable
        .set_input_parameter(
            index(2),
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
        )
        .expect("setTime(int, Time, Calendar)");

    callable
        .execute_prepared_update()
        .await
        .expect("execute prepared update");

    let snapshots = filter.snapshots();
    assert_eq!(snapshots.len(), 1, "执行期必须采集一次参数槽位快照");
    assert_eq!(
        snapshots[0],
        vec![
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Specified(Some(calendar.clone())),
            },
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Specified(Some(calendar)),
            },
        ],
        "执行期槽位必须保留 Calendar 重载描述符"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}
