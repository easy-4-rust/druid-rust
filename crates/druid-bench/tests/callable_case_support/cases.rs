//! 原始 Callable 七例的真实标准调用及可复核观测。
#[path = "mock_driver.rs"]
mod mock_driver;
use crate::standard_test_support::TrackingState;
use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, StandardCallableWasNullFilterChain,
};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{
    CallableParameter, CallableStatement, PreparedStatement, StatementOptions,
};
use druid_rdbc::types::RdbcTargetType;
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
pub const INPUT: &str = include_str!("../fixtures/callable_cases.tsv");
pub const HEADER: &str = "case_id\ttrace\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_before\terrors_after\tactive_during\tidle_during\tstatement_closed\tconnection_closed\tactive_after\tidle_after\treused";
struct WasNullFilter(Arc<AtomicBool>);
#[async_trait::async_trait]
impl BeforeFilter for WasNullFilter {
    fn name(&self) -> &str {
        "original-callable-wasNull-filter"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    fn standard_callable_was_null(
        &self,
        chain: &mut StandardCallableWasNullFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        if self.0.load(Ordering::Acquire) {
            Err(DruidError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            chain.callable_statement_was_null()
        }
    }
}
fn utf16(value: Option<&str>) -> String {
    value.map_or_else(
        || "-".to_owned(),
        |value| {
            let mut s = String::new();
            for unit in value.encode_utf16() {
                write!(s, "{unit:04x}").unwrap();
            }
            s
        },
    )
}
fn diagnostics(result: Result<(), RdbcError>) -> String {
    match result {
        Ok(()) => "RETURN\t-\t-\t-\t-".to_owned(),
        Err(error) => {
            let sql = error.sql_exception();
            format!(
                "THROW\t{}\t{}\t{}\t{}",
                error.class_name(),
                sql.map_or_else(|| "-".to_owned(), |v| v.error_code().to_string()),
                utf16(sql.and_then(SqlException::sql_state)),
                sql.map_or_else(|| utf16(Some(&error.to_string())), |v| utf16(v.message()))
            )
        }
    }
}
fn invoke(
    statement: &dyn CallableStatement,
    input: &[&str],
    raw: usize,
    trace: &mut String,
) -> Result<(), RdbcError> {
    match input[6] {
        "WAS_NULL" => {
            *trace = statement.was_null()?.to_string();
        }
        "TYPED_INDEX" => {
            statement.out_parameter_as(
                &CallableParameter::by_index(input[7].parse().unwrap())?,
                &RdbcTargetType::String,
            )?;
        }
        "TYPED_NAME" => {
            statement.out_parameter_as(
                &CallableParameter::by_name(input[7])?,
                &RdbcTargetType::String,
            )?;
        }
        "UNWRAP" => {
            let StandardUnwrapped::CallableStatement(callable) =
                Wrapper::unwrap(statement, Some(TypeId::of::<dyn CallableStatement>()))?
            else {
                panic!("callable role");
            };
            let callable_address = std::ptr::from_ref(callable).cast::<()>() as usize;
            let role_address = if input[7] == "callable" {
                callable_address
            } else {
                let StandardUnwrapped::PreparedStatement(prepared) =
                    Wrapper::unwrap(statement, Some(TypeId::of::<dyn PreparedStatement>()))?
                else {
                    panic!("prepared role");
                };
                std::ptr::from_ref(prepared).cast::<()>() as usize
            };
            let logical = std::ptr::from_ref(statement).cast::<()>() as usize;
            *trace = format!(
                "role_raw={},callable_raw={},logical={}",
                role_address == raw,
                callable_address == raw,
                role_address == logical && callable_address == logical
            );
        }
        other => panic!("unknown callable action {other}"),
    }
    Ok(())
}
/// 执行固定七例并返回逐项原始观测；不提供差异放行。
pub async fn rust_observations() -> BTreeMap<String, String> {
    let mut records = BTreeMap::new();
    for line in INPUT.lines().skip(1) {
        let input: Vec<_> = line.split('\t').collect();
        assert_eq!(input.len(), 8);
        let state = TrackingState::configured();
        let raw = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(AtomicBool::new(true));
        let mut filters = FilterChain::new();
        if input[4] == "true" {
            filters.add_before(Arc::new(WasNullFilter(Arc::clone(&gate))));
        }
        let pool = DruidPool::new_standard(
            input[0].to_owned(),
            "mock".to_owned(),
            Arc::new(mock_driver::Factory(Arc::clone(&state), Arc::clone(&raw))),
            PoolInnerConfig {
                initial_size: 1,
                ..PoolInnerConfig::default()
            },
            Some(Arc::new(filters)),
        );
        let mut connection = pool.get().await.unwrap();
        let mut statement = connection
            .prepare_call(input[2], StatementOptions::default())
            .await
            .unwrap();
        if input[3] == "true" {
            statement.execute_prepared().await.unwrap();
        }
        gate.store(input[5] == "true", Ordering::Release);
        let before = pool.stat_value_and_reset().error_count;
        let mut trace = "-".to_owned();
        let result = invoke(
            statement.as_ref(),
            &input,
            raw.load(Ordering::Acquire),
            &mut trace,
        );
        let error = diagnostics(result);
        let after = pool.stat_value_and_reset().error_count;
        let during = pool.state();
        statement.close().await.unwrap();
        let statement_closed = statement.is_closed().unwrap();
        connection.close().await.unwrap();
        let connection_closed = connection.is_closed().unwrap();
        let closed = pool.state();
        let mut next = pool.get().await.unwrap();
        let reused = state.factory_create_count.load(Ordering::Acquire) == 1;
        next.close().await.unwrap();
        let row=format!("{trace}\t{error}\t{before}\t{after}\t{}\t{}\t{statement_closed}\t{connection_closed}\t{}\t{}\t{reused}",during.active_count,during.idle_count,closed.active_count,closed.idle_count);
        assert_eq!(row.split('\t').count(), 15);
        assert!(records.insert(input[0].to_owned(), row).is_none());
        pool.close().await;
    }
    assert_eq!(records.len(), 7);
    records
}
