use druid_rdbc::statement::{CallableInputParameter, CallableOutParameter, CallableParameter};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcTargetType, RdbcTypeMap};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
/// 底层 Callable 的真实委托记录，独立于 pooled 的逻辑缓存。对应 Java: java.sql.CallableStatement。
#[derive(Default)]
pub(crate) struct TestCallableState {
    pub(crate) registrations: Mutex<HashMap<CallableParameter, CallableOutParameter>>,
    pub(crate) named_parameters: Mutex<HashMap<String, CallableInputParameter>>,
    pub(crate) calendar_reads: Mutex<Vec<(CallableParameter, RdbcCalendarArgument)>>,
    pub(crate) type_map_reads: Mutex<Vec<(CallableParameter, Option<RdbcTypeMap>)>>,
    pub(crate) typed_reads: Mutex<Vec<(CallableParameter, RdbcTargetType)>>,
    pub(crate) last_was_null: AtomicBool,
}
