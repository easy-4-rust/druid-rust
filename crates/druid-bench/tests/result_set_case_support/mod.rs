//! 原始五例 ResultSet 的 Rust 调用与只读观测。
mod mock_driver;

use crate::standard_test_support::TrackingState;
use druid::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid::stats::{StatFilter, StatsCollector};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetNavigation};
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::{RdbcTargetType, Value};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    Arc,
};

pub const CASES: &str = include_str!("../fixtures/result_set_cases.tsv");
pub const DIGEST: &str = "991d7898de149880a7dc4a4e4dc767d1e5cfdc73f7b2444dbedc7561b8037de6";
pub const HEADER: &str = "case_id\ttrace\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_after\tactive_during\tidle_during\tcursor_closed_after\tstatement_closed_after\tactive_after\tidle_after\treused";

/// 只读取边界 false 时现有上下文；原用例的三次抓取计数均紧随该边界。
#[derive(Default)]
struct BoundarySnapshot {
    peak: AtomicI32,
    at_boundary: AtomicBool,
}
impl BoundarySnapshot {
    fn observe(&self, context: &ResultSetFilterContext, result: &Result<bool, DruidError>) {
        let boundary = matches!(result, Ok(false));
        self.at_boundary.store(boundary, Ordering::Release);
        if boundary {
            self.peak
                .store(context.fetch_row_count(), Ordering::Release);
        }
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for BoundarySnapshot {
    fn standard_result_set_next_after(
        &self,
        context: &ResultSetFilterContext,
        result: &Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        self.observe(context, result);
        Ok(())
    }
    async fn standard_result_set_navigation<'a>(
        &'a self,
        context: &'a ResultSetFilterContext,
        _navigation: ResultSetNavigation,
        next: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<bool, DruidError>> + Send + 'a>,
        >,
    ) -> Result<bool, DruidError> {
        let result = next.await;
        self.observe(context, &result);
        result
    }
}

pub fn cases() -> Vec<Vec<&'static str>> {
    let rows: Vec<_> = CASES
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 8);
            fields
        })
        .collect();
    assert_eq!(rows.len(), 5);
    rows
}

async fn invoke(
    rows: &mut dyn ResultSet,
    input: &[&str],
    boundary: &BoundarySnapshot,
    trace: &mut Vec<String>,
    raw_identity: usize,
) -> Result<(), RdbcError> {
    match input[2] {
        "UNWRAP" => {
            assert_eq!(input[6], "java.sql.ResultSet");
            let StandardUnwrapped::ResultSet(unwrapped) =
                rows.unwrap(Some(TypeId::of::<dyn ResultSet>()))?
            else {
                panic!("canonical ResultSet role expected");
            };
            let unwrapped_address = std::ptr::from_ref(unwrapped).cast::<()>() as usize;
            trace.push(format!("raw={}", unwrapped_address == raw_identity));
            trace.push(format!(
                "logical={}",
                std::ptr::addr_eq(std::ptr::from_ref(unwrapped), std::ptr::from_ref(rows))
            ));
        }
        "TYPED_INDEX" => {
            rows.object_as(
                ColumnIndex::new(input[6].parse().unwrap()).unwrap(),
                &RdbcTargetType::String,
            )?;
        }
        "TYPED_LABEL" => {
            rows.object_as_by_label(input[6], &RdbcTargetType::String)?;
        }
        "NAVIGATION" => {
            for action in input[7].split(',') {
                match action {
                    "next" => trace.push(rows.next().await?.to_string()),
                    "previous" => trace.push(rows.previous().await?.to_string()),
                    "fetch_count" => {
                        assert!(
                            boundary.at_boundary.load(Ordering::Acquire),
                            "original count sample follows false boundary"
                        );
                        trace.push(boundary.peak.load(Ordering::Acquire).to_string());
                    }
                    "row_updated" => trace.push(rows.row_updated()?.to_string()),
                    other => panic!("unknown frozen action {other}"),
                }
            }
        }
        other => panic!("unknown frozen scenario {other}"),
    }
    Ok(())
}

fn utf16(value: Option<&str>) -> String {
    value.map_or_else(
        || "-".to_owned(),
        |value| {
            let mut encoded = String::new();
            for unit in value.encode_utf16() {
                write!(encoded, "{unit:04x}").unwrap();
            }
            encoded
        },
    )
}

pub async fn rust_results() -> BTreeMap<String, String> {
    let mut results = BTreeMap::new();
    for input in cases() {
        let state = TrackingState::configured();
        let boundary = Arc::new(BoundarySnapshot::default());
        let raw_identity = Arc::new(AtomicUsize::new(0));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::clone(&boundary) as Arc<dyn ResultSetFilter>);
        if input[3].parse::<bool>().unwrap() {
            filters.add_filter(Arc::new(StatFilter::new(Arc::new(StatsCollector::new(
                "frozen-result-set",
                std::time::Duration::from_secs(10),
            )))));
        }
        let pool = DruidPool::new_standard(
            input[0].to_owned(),
            "mock".to_owned(),
            Arc::new(mock_driver::MockFactory(
                Arc::clone(&state),
                Arc::clone(&raw_identity),
            )),
            PoolInnerConfig {
                max_open: 1,
                max_idle: 1,
                ..PoolInnerConfig::default()
            },
            Some(Arc::new(filters)),
        );
        pool.init().await.unwrap();
        let mut connection = pool.get().await.unwrap();
        let mut statement = connection
            .prepare_statement(input[4], StatementOptions::default())
            .await
            .unwrap();
        statement
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(input[5].to_owned()),
            })
            .unwrap();
        let mut rows = statement.execute_prepared_query().await.unwrap();
        assert_eq!(pool.stat_value_and_reset().error_count, 0);
        let mut trace = Vec::new();
        let result = invoke(
            rows.as_mut(),
            &input,
            &boundary,
            &mut trace,
            raw_identity.load(Ordering::Acquire),
        )
        .await;
        let (outcome, class, code, sql_state, message) = match result {
            Ok(()) => (
                "RETURN",
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
            ),
            Err(error) => {
                let sql = error.sql_exception();
                (
                    "THROW",
                    error.class_name().to_owned(),
                    sql.map_or_else(|| "-".to_owned(), |sql| sql.error_code().to_string()),
                    utf16(sql.and_then(|sql| sql.sql_state())),
                    sql.map_or_else(
                        || utf16(Some(&error.to_string())),
                        |sql| utf16(sql.message()),
                    ),
                )
            }
        };
        let errors = pool.stat_value_and_reset().error_count;
        let during = pool.state();
        rows.close().await.unwrap();
        connection.close().await.unwrap();
        let cursor_closed = rows.is_closed().unwrap();
        let statement_closed = statement.is_closed().unwrap();
        let after = pool.state();
        let mut next = pool.get().await.unwrap();
        let reused = state.factory_create_count.load(Ordering::Acquire) == 1;
        next.close().await.unwrap();
        let trace = if trace.is_empty() {
            "-".to_owned()
        } else {
            trace.join(",")
        };
        let observation = format!("{trace}\t{outcome}\t{class}\t{code}\t{sql_state}\t{message}\t{errors}\t{}\t{}\t{cursor_closed}\t{statement_closed}\t{}\t{}\t{reused}",
            during.active_count, during.idle_count, after.active_count, after.idle_count);
        assert!(results.insert(input[0].to_owned(), observation).is_none());
        pool.close().await;
    }
    results
}

pub fn raw_fixture_standard_unwrap_preserves_identity() -> bool {
    let cursor = mock_driver::MockCursor::new();
    match cursor.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap() {
        StandardUnwrapped::ResultSet(raw) => {
            std::ptr::addr_eq(std::ptr::from_ref(raw), std::ptr::from_ref(&cursor))
        }
        _ => false,
    }
}
