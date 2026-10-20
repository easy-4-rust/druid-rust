//! 冻结 Pool 六例的标准入口、资源关闭与池状态观测。
use crate::standard_test_support::{TrackingFactory, TrackingState};
use druid::core::{BeforeFilter, ConnectionEvent, DruidError, ExecContext, FilterChain};
use druid::pool::{config::PoolInnerConfig, DruidDataSource, DruidPool};
use druid_rdbc::data_source::DataSource;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{atomic::Ordering, Arc};

pub const INPUT: &str = include_str!("../fixtures/pool_cases.tsv");
pub const HEADER: &str = "case_id\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\tactive_during\tidle_during\tactive_after_close\tidle_after_close\tstatement_closed\tconnection_closed\treplacement_same\tactive_final\tidle_final";

struct RejectRollback;
#[async_trait::async_trait]
impl BeforeFilter for RejectRollback {
    fn name(&self) -> &str {
        "original-pool-rollback-filter"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
    async fn on_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        if matches!(event, ConnectionEvent::Rollback) {
            Err(DruidError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            Ok(())
        }
    }
}
fn utf16(value: Option<&str>) -> String {
    value.map_or_else(
        || "-".to_owned(),
        |value| {
            let mut output = String::new();
            for unit in value.encode_utf16() {
                write!(output, "{unit:04x}").unwrap();
            }
            output
        },
    )
}
fn diagnostics(error: Option<&RdbcError>) -> String {
    error.map_or_else(
        || "RETURN\t-\t-\t-\t-".to_owned(),
        |error| {
            let sql = error.sql_exception();
            format!(
                "THROW\t{}\t{}\t{}\t{}",
                error.class_name(),
                sql.map_or_else(|| "-".to_owned(), |value| value.error_code().to_string()),
                utf16(sql.and_then(SqlException::sql_state)),
                sql.map_or_else(
                    || utf16(Some(&error.to_string())),
                    |value| utf16(value.message())
                )
            )
        },
    )
}

pub async fn rust_observations() -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for line in INPUT.lines().skip(1) {
        let input: Vec<_> = line.split('\t').collect();
        assert_eq!(input.len(), 9);
        let state = TrackingState::configured();
        let mut filters = FilterChain::new();
        if input[6] == "true" {
            filters.add_before(Arc::new(RejectRollback));
        }
        let pool = DruidPool::new_standard(
            input[0].to_owned(),
            "mock".to_owned(),
            Arc::new(TrackingFactory::new(Arc::clone(&state))),
            PoolInnerConfig {
                max_open: input[8].parse().unwrap(),
                max_idle: input[8].parse().unwrap(),
                ..PoolInnerConfig::default()
            },
            Some(Arc::new(filters)),
        );
        let source = DruidDataSource::from_pool(pool);
        let pool = source.native_pool();
        let acquired = if input[1].ends_with("getConnection") {
            source
                .connection_with_nullable_credentials(
                    (input[3] != "-").then_some(input[3]),
                    (input[4] != "-").then_some(input[4]),
                )
                .await
        } else {
            source.connection().await
        };
        let mut active_during = 0;
        let mut idle_during = 0;
        let mut statement_closed = "-".to_owned();
        let mut connection_closed = "-".to_owned();
        let mut same = "-".to_owned();
        let failure = match acquired {
            Err(error) => Some(error),
            Ok(mut connection) => {
                let mut statement = if input[7] != "-" {
                    if input[5] == "false" {
                        connection.set_auto_commit(false).await.unwrap();
                        connection.set_read_only(false).await.unwrap();
                    }
                    let mut statement = connection
                        .create_statement(StatementOptions::default())
                        .await
                        .unwrap();
                    statement
                        .execute(input[7], GeneratedKeys::None)
                        .await
                        .unwrap();
                    Some(statement)
                } else {
                    None
                };
                let during = pool.state();
                active_during = during.active_count;
                idle_during = during.idle_count;
                let failure = connection.close().await.err();
                if let Some(statement) = statement.as_mut() {
                    statement_closed = statement.is_closed().unwrap().to_string();
                }
                connection_closed = connection.is_closed().unwrap().to_string();
                failure
            }
        };
        let after = pool.state();
        if failure.is_none() {
            let mut replacement = source.connection().await.unwrap();
            same = (state.factory_create_count.load(Ordering::Acquire) == 1).to_string();
            replacement.close().await.unwrap();
        }
        let final_state = pool.state();
        let observation = format!("{}\t{active_during}\t{idle_during}\t{}\t{}\t{statement_closed}\t{connection_closed}\t{same}\t{}\t{}",
            diagnostics(failure.as_ref()), after.active_count, after.idle_count, final_state.active_count, final_state.idle_count);
        assert_eq!(observation.split('\t').count(), 14);
        assert!(result.insert(input[0].to_owned(), observation).is_none());
        pool.close().await;
    }
    assert_eq!(result.len(), 6);
    result
}
