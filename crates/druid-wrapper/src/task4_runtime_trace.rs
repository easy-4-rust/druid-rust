//! P04 Task 4 adapter runtime-trace evidence protocol.
//!
//! This module intentionally exposes only an opaque probe, an opaque expectation, and the
//! consuming verifier used by evidence tests. Family identifiers, stages, event construction, and
//! recording remain crate-private so a test or injected I/O double cannot manufacture behavior.

use parking_lot::Mutex;
use serde::Serialize;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const TRACE_VERSION: &str = "task4-runtime-trace-v1";
const TRACE_MARKER: &str = "DRUID_TASK4_RUNTIME_TRACE";
const TRACE_CHALLENGE_ENV: &str = "DRUID_TASK4_TRACE_CHALLENGE";
const STANDALONE_CHALLENGE: &str = "standalone-test-no-verifier";
static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FamilyId {
    Sqlx,
    Toasty,
    Rbdc,
    #[cfg(feature = "duckdb-native")]
    DuckDb,
    #[cfg(feature = "http-sql")]
    HttpSql,
    #[cfg(feature = "libsql-native")]
    LibSql,
    #[cfg(feature = "jdbc-agent")]
    JdbcAgent,
}

impl FamilyId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Sqlx => "SQLX",
            Self::Toasty => "TOASTY",
            Self::Rbdc => "RBDC",
            #[cfg(feature = "duckdb-native")]
            Self::DuckDb => "DUCKDB",
            #[cfg(feature = "http-sql")]
            Self::HttpSql => "HTTP_SQL",
            #[cfg(feature = "libsql-native")]
            Self::LibSql => "LIBSQL",
            #[cfg(feature = "jdbc-agent")]
            Self::JdbcAgent => "JDBC_AGENT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceStage {
    FactoryCreate,
    ConnectionCreate,
    StatementCreate,
    StatementUpdate,
    PreparedCreate,
    ParameterSet,
    PreparedQuery,
    ResultNext,
    ResultSetMetadata,
    DatabaseMetadata,
    ConnectionClose,
}

impl TraceStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::FactoryCreate => "FactoryCreate",
            Self::ConnectionCreate => "ConnectionCreate",
            Self::StatementCreate => "StatementCreate",
            Self::StatementUpdate => "StatementUpdate",
            Self::PreparedCreate => "PreparedCreate",
            Self::ParameterSet => "ParameterSet",
            Self::PreparedQuery => "PreparedQuery",
            Self::ResultNext => "ResultNext",
            Self::ResultSetMetadata => "ResultSetMetadata",
            Self::DatabaseMetadata => "DatabaseMetadata",
            Self::ConnectionClose => "ConnectionClose",
        }
    }
}

const EXPECTED_STAGES: [TraceStage; 11] = [
    TraceStage::FactoryCreate,
    TraceStage::ConnectionCreate,
    TraceStage::StatementCreate,
    TraceStage::StatementUpdate,
    TraceStage::PreparedCreate,
    TraceStage::ParameterSet,
    TraceStage::PreparedQuery,
    TraceStage::ResultNext,
    TraceStage::ResultSetMetadata,
    TraceStage::DatabaseMetadata,
    TraceStage::ConnectionClose,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceEvent {
    family: FamilyId,
    instance_id: u64,
    stage: TraceStage,
    caller_file: &'static str,
    caller_line: u32,
}

#[derive(Debug, Default)]
struct TraceState {
    events: Vec<TraceEvent>,
    emitted: bool,
}

/// Crate-private capability carried only by real factory/adapter execution objects.
#[derive(Clone)]
pub(crate) struct RuntimeTraceEmitter {
    family: FamilyId,
    instance_id: u64,
    state: Arc<Mutex<TraceState>>,
}

impl fmt::Debug for RuntimeTraceEmitter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeTraceEmitter")
            .finish_non_exhaustive()
    }
}

impl RuntimeTraceEmitter {
    #[track_caller]
    fn record(&self, stage: TraceStage) {
        let caller = std::panic::Location::caller();
        self.state.lock().events.push(TraceEvent {
            family: self.family,
            instance_id: self.instance_id,
            stage,
            caller_file: caller.file(),
            caller_line: caller.line(),
        });
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_factory_create(&self) {
        self.record(TraceStage::FactoryCreate);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_connection_create(&self) {
        self.record(TraceStage::ConnectionCreate);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_statement_create(&self) {
        self.record(TraceStage::StatementCreate);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_statement_update(&self) {
        self.record(TraceStage::StatementUpdate);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_prepared_create(&self) {
        self.record(TraceStage::PreparedCreate);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_parameter_set(&self) {
        self.record(TraceStage::ParameterSet);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_prepared_query(&self) {
        self.record(TraceStage::PreparedQuery);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_result_next(&self) {
        self.record(TraceStage::ResultNext);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_result_set_metadata(&self) {
        self.record(TraceStage::ResultSetMetadata);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_database_metadata(&self) {
        self.record(TraceStage::DatabaseMetadata);
    }

    #[inline(never)]
    #[track_caller]
    fn task4_coverage_anchor_connection_close(&self) {
        self.record(TraceStage::ConnectionClose);
    }

    #[track_caller]
    pub(crate) fn task4_trace_factory_create(&self) {
        self.task4_coverage_anchor_factory_create();
    }

    #[track_caller]
    pub(crate) fn task4_trace_connection_create(&self) {
        self.task4_coverage_anchor_connection_create();
    }

    #[track_caller]
    pub(crate) fn task4_trace_statement_create(&self) {
        self.task4_coverage_anchor_statement_create();
    }

    #[track_caller]
    pub(crate) fn task4_trace_statement_update(&self) {
        self.task4_coverage_anchor_statement_update();
    }

    #[track_caller]
    pub(crate) fn task4_trace_prepared_create(&self) {
        self.task4_coverage_anchor_prepared_create();
    }

    #[track_caller]
    pub(crate) fn task4_trace_parameter_set(&self) {
        self.task4_coverage_anchor_parameter_set();
    }

    #[track_caller]
    pub(crate) fn task4_trace_prepared_query(&self) {
        self.task4_coverage_anchor_prepared_query();
    }

    #[track_caller]
    pub(crate) fn task4_trace_result_next(&self) {
        self.task4_coverage_anchor_result_next();
    }

    #[track_caller]
    pub(crate) fn task4_trace_result_set_metadata(&self) {
        self.task4_coverage_anchor_result_set_metadata();
    }

    #[track_caller]
    pub(crate) fn task4_trace_database_metadata(&self) {
        self.task4_coverage_anchor_database_metadata();
    }

    #[track_caller]
    pub(crate) fn task4_trace_connection_close(&self) {
        self.task4_coverage_anchor_connection_close();
    }
}

/// Task 4 evidence test 持有的不透明只读探针。
///
/// 本类型不提供构造器、事件读取、family 读取或记录 API；只能交给
/// [`verify_and_emit`] 做消费式验证。
#[doc(hidden)]
pub struct TraceProbe {
    state: Arc<Mutex<TraceState>>,
}

/// 由真实 family factory 与探针配对的不透明预期值。
///
/// family、instance、version 与 evidence id 字段均不可由测试构造或修改。
#[doc(hidden)]
pub struct TraceExpectation {
    evidence_id: &'static str,
    family: FamilyId,
    instance_id: u64,
}

struct RuntimeTraceSession {
    emitter: RuntimeTraceEmitter,
    probe: TraceProbe,
    expectation: TraceExpectation,
}

impl RuntimeTraceSession {
    fn new(family: FamilyId, evidence_id: &'static str) -> Self {
        let instance_id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
        let state = Arc::new(Mutex::new(TraceState::default()));
        Self {
            emitter: RuntimeTraceEmitter {
                family,
                instance_id,
                state: Arc::clone(&state),
            },
            probe: TraceProbe { state },
            expectation: TraceExpectation {
                evidence_id,
                family,
                instance_id,
            },
        }
    }

    fn into_parts(self) -> (RuntimeTraceEmitter, TraceProbe, TraceExpectation) {
        (self.emitter, self.probe, self.expectation)
    }
}

macro_rules! family_session {
    ($function:ident, $family:ident, $evidence_id:literal) => {
        pub(crate) fn $function() -> (RuntimeTraceEmitter, TraceProbe, TraceExpectation) {
            RuntimeTraceSession::new(FamilyId::$family, $evidence_id).into_parts()
        }
    };
}

family_session!(sqlx_trace_session, Sqlx, "task4.sqlx.runtime-trace-v1");
family_session!(
    toasty_trace_session,
    Toasty,
    "task4.toasty.runtime-trace-v1"
);
family_session!(rbdc_trace_session, Rbdc, "task4.rbdc.runtime-trace-v1");
#[cfg(feature = "duckdb-native")]
family_session!(
    duckdb_trace_session,
    DuckDb,
    "task4.duckdb.runtime-trace-v1"
);
#[cfg(feature = "http-sql")]
family_session!(
    http_sql_trace_session,
    HttpSql,
    "task4.http-sql.runtime-trace-v1"
);
#[cfg(feature = "libsql-native")]
family_session!(
    libsql_trace_session,
    LibSql,
    "task4.libsql.runtime-trace-v1"
);
#[cfg(feature = "jdbc-agent")]
family_session!(
    jdbc_agent_trace_session,
    JdbcAgent,
    "task4.jdbc-agent.runtime-trace-v1"
);

/// typed runtime trace 验证错误。
///
/// 字段保持私有，测试不能检查或伪造协议状态；`Display` 文本只用于诊断。
#[derive(Debug)]
#[doc(hidden)]
pub struct TraceVerificationError(String);

impl fmt::Display for TraceVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TraceVerificationError {}

#[derive(Serialize)]
struct SerializableTraceEvent {
    family: &'static str,
    instance_id: u64,
    stage: &'static str,
    caller_file: &'static str,
    caller_line: u32,
}

#[derive(Serialize)]
struct SerializableTrace {
    evidence_id: &'static str,
    version: &'static str,
    challenge: String,
    family: &'static str,
    instance_id: u64,
    events: Vec<SerializableTraceEvent>,
}

#[inline(never)]
fn task4_coverage_anchor_validation_success() {
    std::hint::black_box(TRACE_VERSION);
}

/// 验证一条完整 typed trace，并输出唯一机器可读 evidence record。
///
/// 参数 `probe` 与 `expectation` 均被消费；只有 exact typed validation 成功后才会序列化并输出
/// marker。失败时返回 [`TraceVerificationError`]，且不会输出 marker。
#[doc(hidden)]
pub fn verify_and_emit(
    probe: TraceProbe,
    expectation: TraceExpectation,
) -> Result<(), TraceVerificationError> {
    let mut state = probe.state.lock();
    if state.emitted {
        return Err(TraceVerificationError(
            "Task 4 runtime trace was already emitted".to_owned(),
        ));
    }
    if state.events.len() != EXPECTED_STAGES.len() {
        return Err(TraceVerificationError(format!(
            "Task 4 runtime trace has {} events; expected {}",
            state.events.len(),
            EXPECTED_STAGES.len()
        )));
    }
    for (index, (event, expected_stage)) in
        state.events.iter().zip(EXPECTED_STAGES.iter()).enumerate()
    {
        if event.family != expectation.family
            || event.instance_id != expectation.instance_id
            || event.stage != *expected_stage
        {
            return Err(TraceVerificationError(format!(
                "Task 4 runtime trace event {index} does not match its typed expectation"
            )));
        }
    }
    let payload = SerializableTrace {
        evidence_id: expectation.evidence_id,
        version: TRACE_VERSION,
        challenge: std::env::var(TRACE_CHALLENGE_ENV)
            .unwrap_or_else(|_| STANDALONE_CHALLENGE.to_owned()),
        family: expectation.family.as_str(),
        instance_id: expectation.instance_id,
        events: state
            .events
            .iter()
            .map(|event| SerializableTraceEvent {
                family: event.family.as_str(),
                instance_id: event.instance_id,
                stage: event.stage.as_str(),
                caller_file: event.caller_file,
                caller_line: event.caller_line,
            })
            .collect(),
    };
    let encoded = serde_json::to_string(&payload).map_err(|error| {
        TraceVerificationError(format!(
            "Task 4 runtime trace serialization failed: {error}"
        ))
    })?;
    state.emitted = true;
    eprintln!("{TRACE_MARKER}\t{encoded}");
    task4_coverage_anchor_validation_success();
    Ok(())
}
