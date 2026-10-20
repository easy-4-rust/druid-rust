//! 对照 Java Druid 1.2.28 七个属性 setter 及 cancel 的过滤器异常，并验证改写后缓存一致。

mod standard_test_support;

use druid::core::{
    BeforeFilter, DruidError, DruidPooledConnection, ExecContext, FilterChain, SqlException,
    StandardStatementMutationFilterChain, StatementMutation,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{FetchDirection, Statement, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 对应 Java: DruidPooledStatementTest.ErrorFilter；模式切换保留同一语句身份。
struct MutationFilter {
    mode: Arc<AtomicUsize>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    outer: bool,
}

#[async_trait::async_trait]
impl BeforeFilter for MutationFilter {
    fn name(&self) -> &str {
        "java-mutation-filter"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    fn standard_statement_mutation(
        &self,
        chain: &mut StandardStatementMutationFilterChain<'_>,
        mutation: StatementMutation,
    ) -> Result<StatementMutation, DruidError> {
        self.trace.lock().unwrap().push(if self.outer {
            "outer-before"
        } else {
            "inner-before"
        });
        let mode = self.mode.load(Ordering::Acquire);
        let result = if self.outer {
            chain.apply(mutation)
        } else {
            match mode {
                0 => Err(DruidError::SqlException(Box::new(SqlException::new(
                    0, None, None,
                )))),
                1 => chain.apply(match mutation {
                    StatementMutation::SetMaxRows(value) => {
                        StatementMutation::SetMaxRows(value + 1)
                    }
                    StatementMutation::SetQueryTimeout(value) => {
                        StatementMutation::SetQueryTimeout(value + 1)
                    }
                    StatementMutation::SetFetchSize(value) => {
                        StatementMutation::SetFetchSize(value + 1)
                    }
                    other => other,
                }),
                2 => Err(DruidError::UnsupportedOperation {
                    operation: "statement_set_max_rows",
                }),
                3 => chain.apply(StatementMutation::SetMaxRows(-1)),
                4 => chain.apply(StatementMutation::SetFetchSize(5)),
                5 => chain
                    .apply(mutation)
                    .map(|_| StatementMutation::SetMaxRows(999)),
                _ => chain.apply(mutation),
            }
        };
        self.trace.lock().unwrap().push(if self.outer {
            "outer-after"
        } else {
            "inner-after"
        });
        result
    }
}

async fn fixture(
    kind: &str,
    name: &str,
    filters: FilterChain,
) -> (
    DruidPool,
    DruidPooledConnection,
    Box<dyn Statement>,
    Arc<TrackingState>,
) {
    let state = TrackingState::configured();
    let pool = DruidPool::new_standard(
        format!("java-mutation-{name}-{kind}"),
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
    let statement: Box<dyn Statement> = match kind {
        "prepared" => connection
            .prepare_statement("SELECT 1", StatementOptions::default())
            .await
            .unwrap(),
        "callable" => connection
            .prepare_call("SELECT 1", StatementOptions::default())
            .await
            .unwrap(),
        _ => connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap(),
    };
    (pool, connection, statement, state)
}

#[tokio::test]
async fn all_statement_kinds_preserve_eight_filter_sql_errors_and_original_properties() {
    for kind in ["statement", "prepared", "callable"] {
        let mode = Arc::new(AtomicUsize::new(0));
        let trace = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_before(Arc::new(MutationFilter {
            mode,
            trace: Arc::clone(&trace),
            outer: false,
        }));
        let (pool, mut connection, mut statement, state) = fixture(kind, "reject", filters).await;
        for operation in 0..8 {
            // 逐次调用并立即读取统计，避免先构造结果数组导致八次操作提前执行。
            let result = match operation {
                0 => statement.set_max_field_size(0),
                1 => statement.set_max_rows(0),
                2 => statement.set_escape_processing(true),
                3 => statement.set_query_timeout(-1),
                4 => statement.set_cursor_name_nullable(None),
                5 => statement.set_fetch_direction(FetchDirection::DriverValue(0)),
                6 => statement.set_fetch_size(0),
                _ => statement.cancel(),
            };
            let error = result.unwrap_err();
            let sql = error.sql_exception().unwrap();
            assert_eq!(sql.class_name(), "java.sql.SQLException");
            assert_eq!(sql.error_code(), 0);
            assert_eq!(sql.sql_state(), None);
            assert_eq!(pool.stat_value_and_reset().error_count, 1, "{kind}");
        }
        assert_eq!(pool.stat_value_and_reset().error_count, 0, "{kind}");
        assert_eq!(statement.max_field_size().unwrap(), 0);
        assert_eq!(statement.max_rows().unwrap(), 0);
        assert_eq!(statement.query_timeout().unwrap(), 0);
        assert_eq!(statement.fetch_size().unwrap(), 0);
        assert!(state.statement_query_timeouts.lock().unwrap().is_empty());
        assert!(state.cursor_names.lock().unwrap().is_empty());
        let calls = trace.lock().unwrap().len();
        statement.close().await.unwrap();
        assert!(statement.set_max_rows(1).is_err());
        assert_eq!(trace.lock().unwrap().len(), calls);
        connection.close().await.unwrap();
        assert_eq!(pool.state().idle_count, 1);
        let mut next = pool.get().await.unwrap();
        assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
        next.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn rewrites_preserve_wide_values_around_order_and_do_not_mask_filter_errors() {
    for kind in ["statement", "prepared", "callable"] {
        let mode = Arc::new(AtomicUsize::new(1));
        let trace = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for outer in [true, false] {
            filters.add_before(Arc::new(MutationFilter {
                mode: Arc::clone(&mode),
                trace: Arc::clone(&trace),
                outer,
            }));
        }
        let (pool, mut connection, mut statement, state) = fixture(kind, "rewrite", filters).await;
        statement.set_max_rows(i64::from(i32::MAX) + 10).unwrap();
        assert_eq!(statement.max_rows().unwrap(), i64::from(i32::MAX) + 11);
        assert_eq!(
            *trace.lock().unwrap(),
            vec!["outer-before", "inner-before", "inner-after", "outer-after"]
        );
        statement.set_query_timeout(6).unwrap();
        assert_eq!(statement.query_timeout().unwrap(), 7);
        assert_eq!(*state.statement_query_timeouts.lock().unwrap(), vec![7]);
        statement.set_query_timeout(-1).unwrap();
        assert_eq!(statement.query_timeout().unwrap(), 0);
        assert_eq!(*state.statement_query_timeouts.lock().unwrap(), vec![7, 0]);
        statement.set_fetch_size(6).unwrap();
        assert_eq!(statement.fetch_size().unwrap(), 7);
        // 过滤器不支持、负值改写、操作种类替换及虚报生效值均不得提交代理缓存。
        for rejected_mode in [2, 3, 4, 5] {
            mode.store(rejected_mode, Ordering::Release);
            assert!(statement.set_max_rows(20).is_err(), "mode {rejected_mode}");
            assert_eq!(statement.max_rows().unwrap(), i64::from(i32::MAX) + 11);
        }
        assert_eq!(pool.stat_value_and_reset().error_count, 0);
        mode.store(6, Ordering::Release);
        statement.set_cursor_name_nullable(None).unwrap();
        statement.set_cursor_name_nullable(Some("")).unwrap();
        statement.set_cursor_name_nullable(Some("cursor")).unwrap();
        assert_eq!(
            *state.cursor_names.lock().unwrap(),
            vec![None, Some(String::new()), Some("cursor".to_owned())]
        );
        assert!(matches!(
            statement.cancel(),
            Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
                operation: "statement_cancel"
            })
        ));
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}
