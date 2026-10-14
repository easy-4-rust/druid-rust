//! 真实 SQLite 验证类型化 auto 准备入口；列选择另有明确待完成合同。

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;

#[tokio::test]
async fn auto_keys_are_empty_after_non_insert_and_zero_row_insert() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let driver = factory.name().to_owned();
        let mut connection = factory.create().await.unwrap();
        connection
            .exec(
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT UNIQUE)",
                vec![],
            )
            .await
            .unwrap();
        connection
            .exec("INSERT INTO key_item(value) VALUES ('seed')", vec![])
            .await
            .unwrap();
        for (sql, count) in [
            (
                "INSERT INTO key_item(value) VALUES ('seed') ON CONFLICT(value) DO NOTHING",
                0,
            ),
            (
                "UPDATE key_item SET value = 'updated' WHERE value = 'seed'",
                1,
            ),
            ("DELETE FROM key_item WHERE value = 'updated'", 1),
        ] {
            let mut prepared = connection
                .prepare_statement_with_generated_keys(
                    sql,
                    StatementOptions::default(),
                    GeneratedKeys::ReturnGeneratedKeys,
                )
                .await
                .unwrap();
            assert_eq!(prepared.execute_prepared_update().await.unwrap(), count);
            let mut keys = prepared.generated_keys().await.unwrap();
            assert!(
                !keys.next().await.unwrap(),
                "driver={driver}, sql={sql}: 非插入和零行插入不能返回历史键"
            );
            keys.close().await.unwrap();
            prepared.close().await.unwrap();
        }
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn cached_auto_statement_has_no_results_from_previous_logical_lease() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let driver = factory.name().to_owned();
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(2)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec(
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT)",
                vec![],
            )
            .await
            .unwrap();
        let sql = "INSERT INTO key_item(value) VALUES (?1)";
        let mut previous = connection
            .prepare_statement_with_generated_keys(
                sql,
                StatementOptions::default(),
                GeneratedKeys::ReturnGeneratedKeys,
            )
            .await
            .unwrap();
        previous
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String("previous".into()),
            })
            .unwrap();
        assert_eq!(previous.execute_prepared_update().await.unwrap(), 1);
        // 不消费键，直接归还缓存；新租约必须具有独立执行状态。
        previous.close().await.unwrap();
        let mut current = connection
            .prepare_statement_with_generated_keys(
                sql,
                StatementOptions::default(),
                GeneratedKeys::ReturnGeneratedKeys,
            )
            .await
            .unwrap();
        assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
        assert_eq!(current.get_update_count().unwrap(), None);
        let mut keys = current.generated_keys().await.unwrap();
        assert!(
            !keys.next().await.unwrap(),
            "driver={driver}: 新租约不能读到前任键"
        );
        keys.close().await.unwrap();
        current
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String("current".into()),
            })
            .unwrap();
        assert_eq!(current.execute_prepared_update().await.unwrap(), 1);
        let mut keys = current.generated_keys().await.unwrap();
        assert!(keys.next().await.unwrap());
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(2)
        );
        keys.close().await.unwrap();
        current.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn auto_prepare_returns_real_sqlite_keys_and_cache_reuse_preserves_request() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let driver = factory.name().to_owned();
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(4)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec(
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)",
                vec![],
            )
            .await
            .unwrap();
        let sql = "INSERT INTO key_item(value) VALUES (?1)";
        for (value, keys) in [
            (1, GeneratedKeys::None),
            (2, GeneratedKeys::ReturnGeneratedKeys),
            (3, GeneratedKeys::ReturnGeneratedKeys),
        ] {
            let mut prepared = connection
                .prepare_statement_with_generated_keys(sql, StatementOptions::default(), keys)
                .await
                .unwrap_or_else(|error| {
                    panic!("driver={driver}, id={value}, prepare error={error:?}")
                });
            prepared
                .set_parameter(PreparedParameter {
                    index: ParameterIndex::new(1).unwrap(),
                    value: Value::String(format!("value-{value}")),
                })
                .unwrap();
            let results = prepared.execute_prepared().await.unwrap();
            assert!(matches!(
                results.as_slice(),
                [ExecutionResult::UpdateCount(1)]
            ));
            assert_eq!(prepared.get_update_count().unwrap(), Some(1));
            let mut generated = prepared.generated_keys().await.unwrap();
            if value == 1 && driver == "sqlx" {
                // SQLx 的 plain 准备没有请求键；保留其既有行为，不能把 None 改成 auto。
                assert!(!generated.next().await.unwrap());
                prepared.close().await.unwrap();
                continue;
            }
            assert!(
                generated.next().await.unwrap(),
                "driver={driver}, inserted id={value}"
            );
            assert_eq!(
                generated.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::Int(value)
            );
            assert!(!generated.next().await.unwrap());
            prepared.close().await.unwrap();
            assert!(
                generated.is_closed().unwrap(),
                "prepared close 必须同时关闭 generated-keys cursor"
            );
        }
        assert_eq!(pool.state().cached_prepared_statement_count, 2);
        assert_eq!(pool.state().cached_prepared_statement_hit_count, 1);
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn sqlx_unsupported_column_selection_does_not_insert_or_enter_prepared_cache() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories
        .into_iter()
        .filter(|factory| factory.name() == "sqlx")
    {
        let pool = DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(4)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec(
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)",
                vec![],
            )
            .await
            .unwrap();
        for keys in [
            GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
            GeneratedKeys::ColumnNames(vec!["id".into()]),
        ] {
            let result = connection
                .prepare_statement_with_generated_keys(
                    "INSERT INTO key_item(value) VALUES ('must-not-insert')",
                    StatementOptions::default(),
                    keys,
                )
                .await;
            assert!(matches!(
                result,
                Err(RdbcError::SqlException(error))
                    if error.sql_state() == Some("0A000")
            ));
            assert_eq!(pool.state().cached_prepared_statement_count, 0);
        }
        let rows = connection
            .fetch("SELECT COUNT(*) FROM key_item", vec![])
            .await
            .unwrap();
        assert_eq!(rows[0].values[0], Value::Int(0));
        // 被拒绝的准备请求不能毒化连接；随后 auto insert 必须真正可用。
        let mut prepared = connection
            .prepare_statement_with_generated_keys(
                "INSERT INTO key_item(value) VALUES ('valid')",
                StatementOptions::default(),
                GeneratedKeys::ReturnGeneratedKeys,
            )
            .await
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        let mut keys = prepared.generated_keys().await.unwrap();
        assert!(keys.next().await.unwrap());
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(1)
        );
        prepared.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn toasty_auto_request_survives_prepared_update_and_batch_execution() {
    prepared_update_and_batch_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn toasty_prepared_column_requests_preserve_base_sqlite_rowid_semantics() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .factory(Arc::new(factory))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection
        .exec(
            "CREATE TABLE column_key_item(id INTEGER PRIMARY KEY, value TEXT)",
            vec![],
        )
        .await
        .unwrap();
    // BASE Toasty SQLite 的 prepared 重载接受列描述，返回单 rowid；不承诺任意列投影。
    let requests = [
        GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        GeneratedKeys::ColumnNames(vec!["id".into()]),
        GeneratedKeys::ColumnIndexes(vec![
            ColumnIndex::new(2).unwrap(),
            ColumnIndex::new(1).unwrap(),
        ]),
        GeneratedKeys::ColumnNames(vec!["value".into(), "id".into()]),
    ];
    for (offset, request) in requests.into_iter().enumerate() {
        let mut prepared = connection
            .prepare_statement_with_generated_keys(
                "INSERT INTO column_key_item(value) VALUES (?1)",
                StatementOptions::default(),
                request,
            )
            .await
            .expect("BASE prepared 列请求必须接受");
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(format!("value-{offset}")),
            })
            .unwrap();
        let results = prepared.execute_prepared().await.unwrap();
        assert!(matches!(
            results.as_slice(),
            [ExecutionResult::UpdateCount(1)]
        ));
        assert_eq!(prepared.get_update_count().unwrap(), Some(1));
        let mut keys = prepared.generated_keys().await.unwrap();
        assert_eq!(keys.meta_data().unwrap().column_count(), 1);
        assert!(keys.next().await.unwrap());
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(i64::try_from(offset + 1).unwrap())
        );
        assert!(!keys.next().await.unwrap());
        prepared.close().await.unwrap();
        assert!(keys.is_closed().unwrap());
    }
    assert_eq!(pool.state().cached_prepared_statement_count, 4);
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn sqlx_auto_request_survives_prepared_update_and_batch_execution() {
    prepared_update_and_batch_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

async fn prepared_update_and_batch_keys(factory: impl ConnectionFactory) {
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)",
            vec![],
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement_with_generated_keys(
            "INSERT INTO key_item(value) VALUES (?1)",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::String("first".into()),
        })
        .unwrap();
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert_eq!(prepared.get_update_count().unwrap(), Some(1));
    let mut first = prepared.generated_keys().await.unwrap();
    assert!(first.next().await.unwrap());
    assert_eq!(
        first.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::String("second".into()),
        })
        .unwrap();
    prepared.add_prepared_batch().unwrap();
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), vec![1]);
    let mut second = prepared.generated_keys().await.unwrap();
    assert!(second.next().await.unwrap());
    assert_eq!(
        second.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(2)
    );
    let rows = connection
        .fetch("SELECT COUNT(*) FROM key_item", vec![])
        .await
        .unwrap();
    assert_eq!(rows[0].values[0], Value::Int(2));
    prepared.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn sqlx_auto_prepare_with_returning_preserves_both_rows_and_generated_key() {
    prepared_returning_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

#[tokio::test]
async fn toasty_auto_prepare_with_returning_preserves_both_rows_and_generated_key() {
    prepared_returning_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

async fn prepared_returning_keys(factory: impl ConnectionFactory) {
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)",
            vec![],
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement_with_generated_keys(
            "INSERT INTO key_item(value) VALUES (?1) RETURNING value, id",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    for id in 1..=2 {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(format!("return-{id}")),
            })
            .unwrap();
        let mut results = prepared.execute_prepared().await.unwrap();
        assert_eq!(prepared.get_update_count().unwrap(), None);
        let ExecutionResult::ResultSet(mut rows) = results.remove(0) else {
            panic!("RETURNING 必须保留结果游标");
        };
        let mut keys = prepared.generated_keys().await.unwrap();
        assert!(
            keys.next().await.unwrap(),
            "RETURNING 分支必须保留 auto 请求"
        );
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(id)
        );
        assert!(
            rows.next().await.unwrap(),
            "获取键不能提前消费原 RETURNING 游标"
        );
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String(format!("return-{id}"))
        );
        assert_eq!(
            rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
            Value::Int(id)
        );
        assert!(!rows.next().await.unwrap());
        rows.close().await.unwrap();
        keys.close().await.unwrap();
    }
    prepared.close().await.unwrap();
    let mut query = connection
        .prepare_statement_with_generated_keys(
            "SELECT id FROM key_item",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let _rows = query.execute_prepared().await.unwrap();
    assert!(
        !query.generated_keys().await.unwrap().next().await.unwrap(),
        "纯 SELECT 不能返回连接上前次插入的键"
    );
    query.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn sqlx_prepared_query_keeps_returning_keys_and_clears_them_on_ignored_insert() {
    prepared_query_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

#[tokio::test]
async fn toasty_prepared_query_keeps_returning_keys_and_clears_them_on_ignored_insert() {
    prepared_query_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

async fn prepared_query_keys(factory: impl ConnectionFactory) {
    let mut connection = factory.create().await.unwrap();
    connection.exec("CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL UNIQUE)", vec![]).await.unwrap();
    let mut prepared = connection.prepare_statement_with_generated_keys(
        "/* 键不能取自 RETURNING 的第一列 */ INSERT INTO key_item(value) VALUES (?1) ON CONFLICT(value) DO NOTHING RETURNING value, id",
        StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys,
    ).await.unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::String("unique".into()),
        })
        .unwrap();
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    let mut keys = prepared.generated_keys().await.unwrap();
    assert!(
        keys.next().await.unwrap(),
        "execute_prepared_query 必须保留 auto 请求"
    );
    assert_eq!(
        keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(1)
    );
    assert!(!keys.next().await.unwrap());
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::String("unique".into())
    );
    rows.close().await.unwrap();
    keys.close().await.unwrap();
    // 重用同一句柄但未插入行时，必须清除上一轮键，不能返回连接级旧值。
    let mut ignored = prepared.execute_prepared_query().await.unwrap();
    assert!(!ignored.next().await.unwrap());
    let mut empty_keys = prepared.generated_keys().await.unwrap();
    assert!(
        !empty_keys.next().await.unwrap(),
        "忽略插入不得泄露旧生成键"
    );
    ignored.close().await.unwrap();
    empty_keys.close().await.unwrap();
    prepared.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn sqlx_failed_prepared_attempt_clears_previous_keys_before_parameter_materialization() {
    failed_prepared_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

#[tokio::test]
async fn toasty_failed_prepared_attempt_clears_previous_keys_before_parameter_materialization() {
    failed_prepared_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

async fn failed_prepared_keys(factory: impl ConnectionFactory) {
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT UNIQUE)",
            vec![],
        )
        .await
        .unwrap();
    for query in [false, true] {
        let sql = if query {
            "INSERT INTO key_item(value) VALUES (?1) RETURNING id"
        } else {
            "INSERT INTO key_item(value) VALUES (?1)"
        };
        let mut prepared = connection
            .prepare_statement_with_generated_keys(
                sql,
                StatementOptions::default(),
                GeneratedKeys::ReturnGeneratedKeys,
            )
            .await
            .unwrap();
        for parameter_failure in [false, true] {
            let value = format!("case-{query}-{parameter_failure}");
            prepared
                .set_parameter(PreparedParameter {
                    index: ParameterIndex::new(1).unwrap(),
                    value: Value::String(value),
                })
                .unwrap();
            if query {
                prepared
                    .execute_prepared_query()
                    .await
                    .unwrap()
                    .close()
                    .await
                    .unwrap();
            } else {
                assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
            }
            // 不读取成功执行的键；下一次失败尝试必须先使它失效。
            if parameter_failure {
                prepared.clear_parameters().unwrap();
            }
            let failed = if query {
                prepared.execute_prepared_query().await.map(|_| ())
            } else {
                prepared.execute_prepared_update().await.map(|_| ())
            };
            assert!(failed.is_err());
            let mut keys = prepared.generated_keys().await.unwrap();
            assert!(
                !keys.next().await.unwrap(),
                "query={query}, parameter_failure={parameter_failure}: 失败执行不得暴露上次键"
            );
            keys.close().await.unwrap();
        }
        prepared.close().await.unwrap();
    }
    connection.close().await.unwrap();
}

#[tokio::test]
async fn toasty_prepared_batch_preserves_all_keys_and_discards_failed_attempt_results() {
    prepared_batch_all_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn sqlx_prepared_batch_preserves_all_keys_and_discards_failed_attempt_results() {
    prepared_batch_all_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

async fn prepared_batch_all_keys(factory: impl ConnectionFactory) {
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "CREATE TABLE key_item(id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT UNIQUE)",
            vec![],
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement_with_generated_keys(
            "INSERT INTO key_item(value) VALUES (?1)",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    for value in ["one", "two", "three"] {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(value.into()),
            })
            .unwrap();
        prepared.add_prepared_batch().unwrap();
    }
    assert_eq!(
        prepared.execute_prepared_batch().await.unwrap(),
        vec![1, 1, 1]
    );
    let mut keys = prepared.generated_keys().await.unwrap();
    for id in [1, 2, 3] {
        assert!(keys.next().await.unwrap(), "批量结果缺少键 {id}");
        assert_eq!(
            keys.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::Int(id)
        );
    }
    assert!(!keys.next().await.unwrap());
    keys.close().await.unwrap();
    for value in ["four", "one", "must-not-run"] {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(value.into()),
            })
            .unwrap();
        prepared.add_prepared_batch().unwrap();
    }
    let failure = prepared.execute_prepared_batch().await.unwrap_err();
    assert!(
        matches!(failure, RdbcError::BatchUpdateException { ref update_counts, .. } if update_counts.large() == [1])
    );
    assert!(prepared.get_update_count().unwrap().is_none());
    let mut keys = prepared.generated_keys().await.unwrap();
    assert!(
        !keys.next().await.unwrap(),
        "失败批次不能发布部分执行或上一批次的键"
    );
    keys.close().await.unwrap();
    assert_eq!(
        prepared.execute_prepared_batch().await.unwrap(),
        Vec::<i64>::new()
    );
    assert!(!prepared
        .generated_keys()
        .await
        .unwrap()
        .next()
        .await
        .unwrap());
    let rows = connection
        .fetch("SELECT COUNT(*) FROM key_item", vec![])
        .await
        .unwrap();
    assert_eq!(rows[0].values[0], Value::Int(4));
    prepared.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn sqlx_returning_keys_identify_current_upsert_and_all_inserted_rows() {
    returning_current_row_keys(SqlxConnectionFactory::new("sqlite::memory:")).await;
}

#[tokio::test]
async fn toasty_returning_keys_identify_current_upsert_and_all_inserted_rows() {
    returning_current_row_keys(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await;
}

async fn returning_current_row_keys(factory: impl ConnectionFactory) {
    for generic in [false, true] {
        for shadowed in [false, true] {
            let mut connection = factory.create().await.unwrap();
            let ddl = if shadowed {
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY, value TEXT UNIQUE, rowid TEXT DEFAULT 'decoy', _rowid_ TEXT DEFAULT 'decoy', oid TEXT DEFAULT 'decoy')"
            } else {
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY, value TEXT UNIQUE)"
            };
            connection.exec(ddl, vec![]).await.unwrap();
            connection
                .exec(
                    "INSERT INTO key_item(value) VALUES ('seed'), ('unrelated')",
                    vec![],
                )
                .await
                .unwrap();
            for (sql, ids, values) in [
                ("INSERT INTO key_item(value) VALUES ('seed') ON CONFLICT(value) DO UPDATE SET value = 'updated' RETURNING 'decoy' AS _rowid_, value", vec![1], vec!["updated"]),
                ("INSERT INTO key_item(value) VALUES ('third'), ('fourth') RETURNING 'decoy' AS _rowid_, value", vec![3, 4], vec!["third", "fourth"]),
            ] {
                let mut prepared = connection.prepare_statement_with_generated_keys(sql, StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys).await.unwrap();
                let mut rows = if generic {
                    let mut results = prepared.execute_prepared().await.unwrap();
                    let ExecutionResult::ResultSet(rows) = results.remove(0) else { panic!("RETURNING 应返回游标") };
                    rows
                } else {
                    prepared.execute_prepared_query().await.unwrap()
                };
                assert!(prepared.get_update_count().unwrap().is_none());
                let mut keys = prepared.generated_keys().await.unwrap();
                for id in ids {
                    assert!(keys.next().await.unwrap(), "缺少当前行键 {id}");
                    assert_eq!(keys.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::Int(id), "generic={generic}, shadowed={shadowed}: 键必须属于当前 RETURNING 行");
                }
                assert!(!keys.next().await.unwrap());
                for value in values {
                    assert!(rows.next().await.unwrap());
                    assert_eq!(rows.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::String("decoy".into()));
                    assert_eq!(rows.value(ColumnIndex::new(2).unwrap()).unwrap(), Value::String(value.into()));
                    assert!(rows.value(ColumnIndex::new(3).unwrap()).is_err(), "隐藏键列不能泄露到原游标");
                }
                assert!(!rows.next().await.unwrap());
                keys.close().await.unwrap();
                rows.close().await.unwrap();
                prepared.close().await.unwrap();
            }
            connection.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn returning_generated_keys_keep_driver_error_for_dropped_table() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let mut connection = factory.create().await.unwrap();
        connection
            .exec("CREATE TABLE key_item(id INTEGER PRIMARY KEY)", vec![])
            .await
            .unwrap();
        let mut prepared = connection
            .prepare_statement_with_generated_keys(
                "INSERT INTO key_item DEFAULT VALUES RETURNING id",
                StatementOptions::default(),
                GeneratedKeys::ReturnGeneratedKeys,
            )
            .await
            .unwrap();
        connection
            .exec("DROP TABLE key_item", vec![])
            .await
            .unwrap();
        let error = match prepared.execute_prepared_query().await {
            Ok(_) => panic!("目标表已删除，执行应失败"),
            Err(error) => error,
        };
        assert!(
            matches!(error, RdbcError::SqlException(_)),
            "内部元数据查询不能把原驱动错误改为 {error:?}"
        );
        assert!(!prepared
            .generated_keys()
            .await
            .unwrap()
            .next()
            .await
            .unwrap());
        prepared.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn returning_generated_keys_respect_sqlite_rowid_schema_boundaries() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        for (ddl, sql, expected) in [
            ("CREATE TABLE key_item(id INTEGER PRIMARY KEY DESC)", "INSERT INTO key_item(id) VALUES (70) RETURNING id", Some(1)),
            ("CREATE TABLE key_item(id INTEGER PRIMARY KEY) WITHOUT ROWID", "INSERT INTO key_item(id) VALUES (70) RETURNING id", None),
            ("CREATE TABLE \"key'item\"(\"real\"\"id\" INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)", "INSERT INTO \"key'item\" DEFAULT VALUES RETURNING \"real\"\"id\"", Some(1)),
        ] {
            let mut connection = factory.create().await.unwrap();
            connection.exec("CREATE TABLE history(id INTEGER PRIMARY KEY)", vec![]).await.unwrap();
            connection.exec("INSERT INTO history(id) VALUES (999)", vec![]).await.unwrap();
            connection.exec(ddl, vec![]).await.unwrap();
            let mut prepared = connection.prepare_statement_with_generated_keys(sql, StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys).await.unwrap();
            let mut rows = prepared.execute_prepared_query().await.unwrap();
            assert!(rows.next().await.unwrap());
            assert!(rows.value(ColumnIndex::new(2).unwrap()).is_err());
            let mut keys = prepared.generated_keys().await.unwrap();
            if let Some(expected) = expected {
                assert!(keys.next().await.unwrap());
                assert_eq!(keys.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::Int(expected));
            }
            assert!(!keys.next().await.unwrap());
            keys.close().await.unwrap();
            rows.close().await.unwrap();
            prepared.close().await.unwrap();
            connection.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn toasty_returning_keys_follow_sqlite_case_and_schema_resolution() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    returning_keys_follow_sqlite_name_resolution(&factory).await;
}

#[tokio::test]
async fn sqlx_returning_keys_follow_sqlite_case_and_schema_resolution() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    returning_keys_follow_sqlite_name_resolution(&factory).await;
}

async fn returning_keys_follow_sqlite_name_resolution(factory: &dyn ConnectionFactory) {
    for (setup, sql, expected) in [
        (
            vec!["CREATE TABLE mixed_name(id INTEGER PRIMARY KEY)"],
            "INSERT INTO MAIN.MIXED_NAME(id) VALUES (41) RETURNING id, 'visible'",
            41,
        ),
        (
            vec![
                "CREATE TABLE mixed_name(base_key INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)",
                "CREATE TEMP TABLE mixed_name(actual_key INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)",
            ],
            "INSERT INTO MiXeD_NaMe(actual_key) VALUES (71) RETURNING actual_key, 'visible'",
            71,
        ),
        (
            vec![
                "ATTACH DATABASE ':memory:' AS attached_db",
                "CREATE TABLE attached_db.mixed_name(actual_key INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)",
            ],
            "INSERT INTO ATTACHED_DB.MIXED_NAME(actual_key) VALUES (81) RETURNING actual_key, 'visible'",
            81,
        ),
        (
            vec![
                "ATTACH DATABASE ':memory:' AS first_db",
                "ATTACH DATABASE ':memory:' AS second_db",
                "CREATE TABLE first_db.mixed_name(actual_key INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)",
                "CREATE TABLE second_db.mixed_name(other_key INTEGER PRIMARY KEY, rowid TEXT, _rowid_ TEXT, oid TEXT)",
            ],
            "INSERT INTO mixed_name(actual_key) VALUES (91) RETURNING actual_key, 'visible'",
            91,
        ),
    ] {
        let mut connection = factory.create().await.unwrap();
        for command in setup {
            connection.exec(command, vec![]).await.unwrap();
        }
        let mut prepared = connection.prepare_statement_with_generated_keys(
            sql, StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys,
        ).await.unwrap();
        let mut rows = prepared.execute_prepared_query().await.unwrap();
        let mut keys = prepared.generated_keys().await.unwrap();
        assert!(keys.next().await.unwrap(), "SQLite 名称解析成功后不能丢失生成键：{sql}");
        assert_eq!(keys.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::Int(expected));
        assert!(!keys.next().await.unwrap());
        assert!(rows.next().await.unwrap());
        assert_eq!(rows.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::Int(expected));
        assert_eq!(rows.value(ColumnIndex::new(2).unwrap()).unwrap(), Value::String("visible".into()));
        assert!(rows.value(ColumnIndex::new(3).unwrap()).is_err());
        assert!(!rows.next().await.unwrap());
        keys.close().await.unwrap();
        rows.close().await.unwrap();
        prepared.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn toasty_prepared_update_keys_cover_current_upsert_and_multirow_insert() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    prepared_update_keys_cover_current_rows(&factory).await;
}

#[tokio::test]
async fn sqlx_prepared_update_keys_cover_current_upsert_and_multirow_insert() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    prepared_update_keys_cover_current_rows(&factory).await;
}

async fn prepared_update_keys_cover_current_rows(factory: &dyn ConnectionFactory) {
    for mode in ["update", "generic", "batch"] {
        let batch = mode == "batch";
        let mut connection = factory.create().await.unwrap();
        connection
            .exec(
                "CREATE TABLE key_item(id INTEGER PRIMARY KEY, value TEXT UNIQUE)",
                vec![],
            )
            .await
            .unwrap();
        connection
            .exec(
                "INSERT INTO key_item(value) VALUES ('seed'), ('unrelated')",
                vec![],
            )
            .await
            .unwrap();
        for (sql, count, expected) in [
            ("INSERT INTO key_item(value) VALUES ('seed') ON CONFLICT(value) DO UPDATE SET value = 'updated'", 1, vec![1]),
            ("INSERT INTO key_item(value) VALUES ('third'), ('fourth')", 2, vec![3, 4]),
            ("INSERT OR IGNORE INTO key_item(value) VALUES ('third')", 0, vec![]),
            ("INSERT INTO key_item(id, value) VALUES (0, 'zero')", 1, vec![0]),
        ] {
            let mut prepared = connection.prepare_statement_with_generated_keys(sql, StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys).await.unwrap();
            if batch {
                prepared.add_prepared_batch().unwrap();
                assert_eq!(prepared.execute_prepared_batch().await.unwrap(), vec![count]);
            } else if mode == "generic" {
                let results = prepared.execute_prepared().await.unwrap();
                assert_eq!(results.len(), 1);
                assert!(matches!(results.as_slice(), [ExecutionResult::UpdateCount(actual)] if *actual == count));
                assert_eq!(prepared.get_update_count().unwrap(), Some(count));
            } else {
                assert_eq!(prepared.execute_prepared_update().await.unwrap(), count);
                assert_eq!(prepared.get_update_count().unwrap(), Some(count));
            }
            let mut keys = prepared.generated_keys().await.unwrap();
            for expected in expected {
                assert!(keys.next().await.unwrap(), "缺少当前执行键：{sql}");
                assert_eq!(keys.value(ColumnIndex::new(1).unwrap()).unwrap(), Value::Int(expected), "batch={batch}, SQL={sql}");
            }
            assert!(!keys.next().await.unwrap());
            keys.close().await.unwrap();
            prepared.close().await.unwrap();
        }
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn prepared_update_keys_preserve_actual_changes_and_unavailable_rowid() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        for (setup, sql, count, view) in [
            (vec!["CREATE TABLE key_item(id INTEGER PRIMARY KEY) WITHOUT ROWID"], "INSERT INTO key_item(id) VALUES (70)", 1, false),
            (vec!["CREATE TABLE key_sink(id INTEGER PRIMARY KEY)", "CREATE VIEW key_item AS SELECT id FROM key_sink", "CREATE TRIGGER insert_view INSTEAD OF INSERT ON key_item BEGIN INSERT INTO key_sink(id) VALUES (NEW.id); END"], "INSERT INTO key_item(id) VALUES (70)", 0, true),
            (vec!["CREATE TABLE key_item(id INTEGER PRIMARY KEY)", "CREATE TRIGGER ignore_insert BEFORE INSERT ON key_item BEGIN SELECT RAISE(IGNORE); END"], "INSERT INTO key_item(id) VALUES (70)", 0, false),
        ] {
            let mut connection = factory.create().await.unwrap();
            for command in setup { connection.exec(command, vec![]).await.unwrap(); }
            let mut prepared = connection.prepare_statement_with_generated_keys(sql, StatementOptions::default(), GeneratedKeys::ReturnGeneratedKeys).await.unwrap();
            assert_eq!(prepared.execute_prepared_update().await.unwrap(), count, "更新数必须来自 changes()：{sql}");
            assert_eq!(prepared.get_update_count().unwrap(), Some(count));
            let mut keys = prepared.generated_keys().await.unwrap();
            assert!(!keys.next().await.unwrap(), "没有可用 rowid 时不能发布历史键：{sql}");
            keys.close().await.unwrap();
            prepared.close().await.unwrap();
            if view {
                // 视图触发器实际写入底表，即使外层 changes 为零。
                let rows = connection.fetch("SELECT id FROM key_sink", vec![]).await.unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].values, vec![Value::Int(70)]);
            }
            connection.close().await.unwrap();
        }
    }
}
