//! 固定 Java Statement 错误语料的真实 Rust 调用及池状态观测。

use druid::core::{
    BeforeFilter, DruidError, ExecContext, FilterChain, SqlException, SqlWarning,
    StandardStatementClearBatchFilterChain, StandardStatementMutationFilterChain,
    StandardStatementWarningFilterChain, StatementMutation, StatementPropertyEvent,
};
use druid::pool::{config::PoolInnerConfig, DruidPool};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CurrentResultDisposition, FetchDirection, GeneratedKeys, Statement, StatementOptions,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{atomic::Ordering, Arc};

use crate::standard_test_support::{TrackingFactory, TrackingState};

pub const CASES: &str = include_str!("../fixtures/statement_error_cases.tsv");
pub const DIGEST: &str = "2fa81686348ffe8abe951181735fb1c3916d9df0038267fb87177d9ea5e1f04d";
pub const HEADER: &str = "case_id\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_before\tactive_before\tidle_before\terrors_after\tactive_during\tidle_during\tactive_after\tidle_after\treused";

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；在对应真实入口抛出空字段 SQLException。
struct StatementErrorFilter;

fn rejection() -> DruidError {
    DruidError::SqlException(Box::new(SqlException::new(0, None, None)))
}

#[async_trait::async_trait]
impl BeforeFilter for StatementErrorFilter {
    fn name(&self) -> &str {
        "frozen-statement-error-filter"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Err(rejection())
    }
    fn standard_statement_property_read(
        &self,
        _event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        Err(rejection())
    }
    fn statement_add_batch_sql(&self, _sql: &str) -> Result<String, DruidError> {
        Err(rejection())
    }
    fn standard_statement_clear_batch(
        &self,
        _chain: &mut StandardStatementClearBatchFilterChain<'_>,
    ) -> Result<(), DruidError> {
        Err(rejection())
    }
    fn standard_statement_mutation(
        &self,
        chain: &mut StandardStatementMutationFilterChain<'_>,
        mutation: StatementMutation,
    ) -> Result<StatementMutation, DruidError> {
        if mutation == StatementMutation::CloseOnCompletion {
            chain.apply(mutation)
        } else {
            Err(rejection())
        }
    }
    fn standard_statement_get_warnings(
        &self,
        _chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        Err(rejection())
    }
    fn standard_statement_clear_warnings(
        &self,
        _chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        Err(rejection())
    }
}

pub fn cases() -> Vec<Vec<&'static str>> {
    let rows: Vec<_> = CASES
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 6, "six frozen input fields");
            fields
        })
        .collect();
    assert_eq!(rows.len(), 37, "fixed Java test denominator");
    rows
}

fn keys(fields: &[&str]) -> GeneratedKeys {
    match fields[2] {
        "TEXT" => GeneratedKeys::None,
        "SQL_INT" => {
            assert_eq!(fields[4], "1");
            GeneratedKeys::ReturnGeneratedKeys
        }
        "SQL_INT_ARRAY" => {
            assert_eq!(fields[4], "[]");
            GeneratedKeys::ColumnIndexes(Vec::new())
        }
        "SQL_STRING_ARRAY" => {
            assert_eq!(fields[4], "[]");
            GeneratedKeys::ColumnNames(Vec::new())
        }
        other => panic!("unknown generated-key shape {other}"),
    }
}

async fn invoke(statement: &mut dyn Statement, fields: &[&str]) -> Result<(), RdbcError> {
    let int = || fields[3].parse::<i32>().expect("frozen integer input");
    match fields[1] {
        "executeQuery" => statement.execute_query(fields[3]).await.map(|_| ()),
        "executeUpdate" => statement
            .execute_update(fields[3], keys(fields))
            .await
            .map(|_| ()),
        "execute" => statement.execute(fields[3], keys(fields)).await.map(|_| ()),
        "getMaxFieldSize" => statement.max_field_size().map(|_| ()),
        "setMaxFieldSize" => statement.set_max_field_size(int()),
        "getMaxRows" => statement.max_rows().map(|_| ()),
        "setMaxRows" => statement.set_max_rows(i64::from(int())),
        "setEscapeProcessing" => statement.set_escape_processing(fields[3].parse().unwrap()),
        "getQueryTimeout" => statement.query_timeout().map(|_| ()),
        "setQueryTimeout" => statement.set_query_timeout(int()),
        "cancel" => statement.cancel(),
        "getWarnings" => statement.warnings().map(|_| ()),
        "clearWarnings" => statement.clear_warnings(),
        "setCursorName" => {
            statement.set_cursor_name_nullable((fields[3] != "-").then_some(fields[3]))
        }
        "getResultSet" => statement.get_result_set().await.map(|_| ()),
        "getUpdateCount" => statement.get_update_count().map(|_| ()),
        "getMoreResults" => {
            if fields[2] == "INT" {
                statement.get_more_results_current(int()).await.map(|_| ())
            } else {
                assert_eq!(fields[2], "NONE");
                statement
                    .get_more_results(CurrentResultDisposition::Default)
                    .await
                    .map(|_| ())
            }
        }
        "setFetchDirection" => statement.set_fetch_direction(FetchDirection::DriverValue(int())),
        "getFetchDirection" => statement.fetch_direction().map(|_| ()),
        "setFetchSize" => statement.set_fetch_size(int()),
        "getFetchSize" => statement.fetch_size().map(|_| ()),
        "getResultSetConcurrency" => statement.result_set_concurrency().map(|_| ()),
        "getResultSetType" => statement.result_set_type().map(|_| ()),
        "addBatch" => statement.add_batch(fields[3].to_owned()),
        "clearBatch" => statement.clear_batch(),
        "executeBatch" => statement.execute_batch().await.map(|_| ()),
        "getGeneratedKeys" => statement.generated_keys().await.map(|_| ()),
        "getResultSetHoldability" => statement.result_set_holdability().map(|_| ()),
        "closeOnCompletion" => {
            statement.close_on_completion()?;
            assert!(
                statement.is_close_on_completion()?,
                "standard behavior is enabled"
            );
            Ok(())
        }
        "isCloseOnCompletion" => {
            assert!(
                !statement.is_close_on_completion()?,
                "fresh statement default is false"
            );
            Ok(())
        }
        other => panic!("unmapped frozen method {other}"),
    }
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
    let mut records = BTreeMap::new();
    for fields in cases() {
        let state = TrackingState::configured();
        let mut filters = FilterChain::new();
        filters.add_before(Arc::new(StatementErrorFilter));
        let pool = DruidPool::new_standard(
            format!("statement-oracle-{}", fields[0]),
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
        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let errors_before = pool.stat_value_and_reset().error_count;
        let before = pool.state();
        let result = invoke(statement.as_mut(), &fields).await;
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
                    utf16(sql.and_then(SqlException::sql_state)),
                    sql.map_or_else(
                        || utf16(Some(&error.to_string())),
                        |sql| utf16(sql.message()),
                    ),
                )
            }
        };
        let errors_after = pool.stat_value_and_reset().error_count;
        let during = pool.state();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        let after = pool.state();
        let mut next = pool.get().await.unwrap();
        // 单连接工厂只创建过一次物理连接；再次借出不能替换为另一条物理连接。
        let reused = state.factory_create_count.load(Ordering::Acquire) == 1;
        next.close().await.unwrap();
        let row = format!("{outcome}\t{class}\t{code}\t{sql_state}\t{message}\t{errors_before}\t{}\t{}\t{errors_after}\t{}\t{}\t{}\t{}\t{reused}",
            before.active_count, before.idle_count, during.active_count, during.idle_count, after.active_count, after.idle_count);
        assert!(
            records.insert(fields[0].to_owned(), row).is_none(),
            "duplicate Rust case"
        );
        pool.close().await;
    }
    records
}

pub fn approved_difference(id: &str, java: &str, rust: &str) -> bool {
    // 完整预期记录约束两项已批准差异；其他字段漂移仍失败。
    matches!(
        id,
        "test_closeOnCompletion_error" | "test_isCloseOnCompletion_error"
    ) && java
        == "THROW\tjava.sql.SQLFeatureNotSupportedException\t0\t-\t-\t0\t1\t0\t0\t1\t0\t0\t1\ttrue"
        && rust == "RETURN\t-\t-\t-\t-\t0\t1\t0\t0\t1\t0\t0\t1\ttrue"
}
