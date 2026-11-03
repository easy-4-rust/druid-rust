//! `java.sql.CallableStatement` 索引 setter 在 Callable 句柄上的绑定合同。
//!
//! 对应 Java: `java.sql.CallableStatement#setXxx(int, ...)`（继承自
//! `java.sql.PreparedStatement`）。本文件只覆盖索引形 setter：通用
//! `set_parameter` 与类型化 `set_input_parameter`。
//!
//! 合同要点：
//! 1. 索引是 1-based；`ParameterIndex::new(0)` 与 `index_parameter(0)` 都被拒绝。
//! 2. 绑定落在 1-based 索引对应的槽位；调用先后不影响槽位归属。
//! 3. `PreparedInputParameter` 的 variant 身份在池化代理与驱动委托两层都不丢失。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, FilterChain, ResultSetFilter,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement, Statement,
    StatementOptions,
};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, Value};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 驱动层记录到的单次索引绑定。
#[derive(Debug, Clone, PartialEq)]
enum RecordedBinding {
    /// `set_parameter(PreparedParameter)` 的通用值绑定。
    Generic {
        /// 1-based 参数索引。
        index: usize,
        /// 绑定的通用 SQL 值。
        value: Value,
    },
    /// `set_input_parameter(index, PreparedInputParameter)` 的类型化绑定。
    Typed {
        /// 1-based 参数索引。
        index: usize,
        /// 完整保留的 setter 描述符。
        parameter: PreparedInputParameter,
    },
}

impl RecordedBinding {
    /// 返回本次绑定使用的 1-based 参数索引。
    fn index(&self) -> usize {
        match self {
            Self::Generic { index, .. } | Self::Typed { index, .. } => *index,
        }
    }
}

/// 驱动层与池化代理共享的绑定记录。
#[derive(Debug, Default)]
struct RecordingState {
    bindings: Mutex<Vec<RecordedBinding>>,
}

impl RecordingState {
    /// 记录一次驱动层收到的索引绑定。
    fn record(&self, binding: RecordedBinding) {
        self.bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(binding);
    }

    /// 返回当前全部绑定记录的快照。
    fn snapshot(&self) -> Vec<RecordedBinding> {
        self.bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// 记录索引 setter 委托的驱动级 Callable mock。
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

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Ok(0)
    }

    fn set_query_timeout(&mut self, _seconds: i32) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.state.record(RecordedBinding::Generic {
            index: parameter.index.get(),
            value: parameter.value,
        });
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.state.record(RecordedBinding::Typed {
            index: index.get(),
            parameter,
        });
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
        Ok("recording-callable".to_owned())
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
        "callable-indexed-slot-snapshot"
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
        "callable-indexed-slot-snapshot"
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
        .name("callable-indexed-setter")
        .driver_name("recording-callable")
        .factory(Arc::new(RecordingFactory { state }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .expect("recording pool")
}

/// 覆盖的索引 setter 家族：每个描述符对应一个 Java 索引 setter。
fn indexed_variants() -> Vec<(usize, PreparedInputParameter)> {
    let date = NaiveDate::from_ymd_opt(2026, 11, 3).expect("valid date");
    let time = NaiveTime::from_hms_opt(9, 30, 15).expect("valid time");
    let timestamp = NaiveDateTime::new(date, time);
    vec![
        (1, PreparedInputParameter::null(4)),
        (
            2,
            PreparedInputParameter::null_with_type_name(4, Some("MY_UDT".to_owned())),
        ),
        (3, PreparedInputParameter::Boolean(true)),
        (4, PreparedInputParameter::Byte(-8)),
        (5, PreparedInputParameter::Short(1_600)),
        (6, PreparedInputParameter::Int(-32)),
        (7, PreparedInputParameter::Long(9_000_000_000)),
        (8, PreparedInputParameter::Float(1.5)),
        (9, PreparedInputParameter::Double(2.25)),
        (
            10,
            PreparedInputParameter::BigDecimal(Some(
                BigDecimal::from_str("123.450").expect("valid decimal"),
            )),
        ),
        (11, PreparedInputParameter::String(Some("text".to_owned()))),
        (
            12,
            PreparedInputParameter::NString(Some("国家字符".to_owned())),
        ),
        (13, PreparedInputParameter::Bytes(Some(vec![1, 2, 3]))),
        (
            14,
            PreparedInputParameter::Date {
                value: Some(date),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        ),
        (
            15,
            PreparedInputParameter::Time {
                value: Some(time),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        ),
        (
            16,
            PreparedInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: RdbcCalendarArgument::Unspecified,
            },
        ),
        (
            17,
            PreparedInputParameter::object(Some(RdbcObject::Scalar(Value::Int(42)))),
        ),
        (
            18,
            PreparedInputParameter::object_with_sql_type(
                Some(RdbcObject::Scalar(Value::Int(7))),
                4,
            ),
        ),
        (
            19,
            PreparedInputParameter::object_with_sql_type_and_scale(
                Some(RdbcObject::Scalar(Value::Int(9))),
                4,
                2,
            ),
        ),
    ]
}

/// 通用 `set_parameter` 路径按 1-based 索引绑定，并在调用顺序上保持记录顺序。
#[tokio::test]
async fn callable_indexed_set_parameter_binds_one_based_slots_in_call_order() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call indexed_generic(?, ?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    let values = [
        Value::Int(10),
        Value::String("alpha".to_owned()),
        Value::Bool(true),
    ];
    for (offset, value) in values.iter().enumerate() {
        callable
            .set_parameter(PreparedParameter {
                index: index(offset + 1),
                value: value.clone(),
            })
            .expect("indexed set_parameter");
    }

    let recorded = state.snapshot();
    assert_eq!(recorded.len(), 3, "每次 set_parameter 必须产生一条记录");
    for (offset, binding) in recorded.iter().enumerate() {
        assert_eq!(binding.index(), offset + 1, "记录必须保留 1-based 索引");
        assert_eq!(
            binding,
            &RecordedBinding::Generic {
                index: offset + 1,
                value: values[offset].clone(),
            }
        );
    }

    callable
        .execute_prepared_update()
        .await
        .expect("execute prepared update");
    let snapshots = filter.snapshots();
    assert_eq!(snapshots.len(), 1, "执行期必须采集一次参数槽位快照");
    assert_eq!(
        snapshots[0],
        vec![
            PreparedInputParameter::RustValue(Value::Int(10)),
            PreparedInputParameter::RustValue(Value::String("alpha".to_owned())),
            PreparedInputParameter::RustValue(Value::Bool(true)),
        ],
        "通用值必须落在 1-based 索引对应的槽位"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 类型化 `set_input_parameter` 覆盖 19 个 setter 家族，variant 身份逐条保留。
#[tokio::test]
async fn callable_indexed_input_parameter_variants_bind_exact_descriptors() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call indexed_variants(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    let variants = indexed_variants();
    assert!(
        variants.len() >= 12,
        "至少覆盖 12 个不同 input variant，实际 {}",
        variants.len()
    );
    for (position, parameter) in variants.iter() {
        callable
            .set_input_parameter(index(*position), parameter.clone())
            .expect("indexed set_input_parameter");
    }

    let recorded = state.snapshot();
    assert_eq!(
        recorded.len(),
        variants.len(),
        "每次 set_input_parameter 必须产生一条记录"
    );
    for (offset, binding) in recorded.iter().enumerate() {
        let (position, parameter) = &variants[offset];
        assert_eq!(
            binding,
            &RecordedBinding::Typed {
                index: *position,
                parameter: parameter.clone(),
            },
            "variant 身份必须在驱动委托层完整保留"
        );
    }

    callable
        .execute_prepared_update()
        .await
        .expect("execute prepared update");
    let snapshots = filter.snapshots();
    assert_eq!(snapshots.len(), 1, "执行期必须采集一次参数槽位快照");
    let expected: Vec<PreparedInputParameter> = variants
        .iter()
        .map(|(_, parameter)| parameter.clone())
        .collect();
    assert_eq!(
        snapshots[0], expected,
        "每个描述符必须落在 1-based 索引对应的槽位"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 交错设置 index 2 再 index 1 时，槽位归属仍按 1-based 索引而非调用顺序。
#[tokio::test]
async fn callable_indexed_setters_place_bindings_by_one_based_index() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut callable = connection
        .prepare_call(
            "{call indexed_interleaved(?, ?, ?)}",
            StatementOptions::default(),
        )
        .await
        .expect("callable handle");

    callable
        .set_input_parameter(index(2), PreparedInputParameter::Int(200))
        .expect("bind index 2 first");
    callable
        .set_input_parameter(index(1), PreparedInputParameter::Long(100))
        .expect("bind index 1 second");
    callable
        .set_parameter(PreparedParameter {
            index: index(3),
            value: Value::String("third".to_owned()),
        })
        .expect("bind index 3");

    let recorded = state.snapshot();
    assert_eq!(
        recorded
            .iter()
            .map(RecordedBinding::index)
            .collect::<Vec<_>>(),
        vec![2, 1, 3],
        "驱动层记录必须保持调用顺序"
    );
    assert_eq!(
        recorded[0],
        RecordedBinding::Typed {
            index: 2,
            parameter: PreparedInputParameter::Int(200),
        }
    );
    assert_eq!(
        recorded[1],
        RecordedBinding::Typed {
            index: 1,
            parameter: PreparedInputParameter::Long(100),
        }
    );

    callable
        .execute_prepared_update()
        .await
        .expect("execute prepared update");
    let snapshots = filter.snapshots();
    assert_eq!(
        snapshots[0],
        vec![
            PreparedInputParameter::Long(100),
            PreparedInputParameter::Int(200),
            PreparedInputParameter::RustValue(Value::String("third".to_owned())),
        ],
        "槽位归属必须由 1-based 索引决定，与调用顺序无关"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 索引 0 不是合法的 JDBC 参数索引。
#[tokio::test]
async fn callable_indexed_setter_rejects_zero_based_index() {
    let state = Arc::new(RecordingState::default());
    let filter = Arc::new(SlotSnapshotFilter::default());
    let pool = recording_pool(Arc::clone(&state), Arc::clone(&filter)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let callable = connection
        .prepare_call("{call indexed_zero()}", StatementOptions::default())
        .await
        .expect("callable handle");

    assert!(
        ParameterIndex::new(0).is_err(),
        "0 不是合法的 1-based 参数索引"
    );
    assert!(
        matches!(
            callable.index_parameter(0),
            Err(RdbcError::InvalidArgument(_))
        ),
        "Callable 句柄必须拒绝 0-based 参数索引"
    );
    assert!(
        callable.index_parameter(1).is_ok(),
        "1-based 参数索引必须被接受"
    );

    connection.close().await.expect("close connection");
    pool.close().await;
}
