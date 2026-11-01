//! 快捷体验：一键启动一个灌好示例数据的 Druid Admin，直接打开控制台。
//!
//! ```sh
//! cargo run -p druid-admin --example console_demo
//! # 然后浏览器访问 http://127.0.0.1:8080/druid/
//! ```
//!
//! 数据是进程内的示例统计（两个数据源、若干 SQL 统计、一条 Wall 汇总），
//! 仅用于人工核对控制台页面，不接任何真实数据库、不做持久化。

use std::collections::HashMap;

use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};
use druid_metrics::ServiceIdentity;
use serde_json::{json, Value};

const LISTEN: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let repository = MetricsRepository::new();

    for (
        service,
        instance,
        boot,
        identity,
        name,
        db_type,
        driver,
        url,
        user,
        active,
        pooling,
        max_active,
        execute_count,
    ) in [
        (
            "order-service",
            "node-1",
            "boot-1",
            1,
            "order-datasource",
            "postgresql",
            "postgres",
            "jdbc:postgresql://10.0.0.7:5432/orders",
            "orders_app",
            12,
            8,
            64,
            9482,
        ),
        (
            "inventory-service",
            "node-2",
            "boot-2",
            2,
            "inventory-datasource",
            "mysql",
            "mysql",
            "jdbc:mysql://10.0.0.9:3306/inventory",
            "inventory_app",
            5,
            20,
            48,
            15230,
        ),
    ] {
        let service_identity =
            ServiceIdentity::try_new(service, instance, boot).expect("valid identity");
        let entry = DataSourceEntry {
            datasource: DataSourceContent {
                service_id: Some(service_identity.service_id().into()),
                service_name: Some(service.into()),
                instance_id: Some(instance.into()),
                boot_id: Some(boot.into()),
                identity,
                name: Some(name.into()),
                db_type: Some(db_type.into()),
                driver_class_name: Some(driver.into()),
                url: Some(url.into()),
                user_name: Some(user.into()),
                active_count: active,
                active_peak: active + 4,
                active_peak_time: Some("2026-11-01 09:15:00".into()),
                pooling_count: pooling,
                pooling_peak: pooling + 6,
                pooling_peak_time: Some("2026-11-01 08:50:00".into()),
                max_active,
                initial_size: 4,
                min_idle: 6,
                max_wait: 60_000,
                max_wait_thread_count: 64,
                wait_thread_count: 0,
                not_empty_wait_count: execute_count / 500,
                not_empty_wait_millis: execute_count / 20,
                test_on_borrow: true,
                test_on_return: false,
                test_while_idle: true,
                keep_alive: true,
                fail_fast: false,
                remove_abandoned: true,
                pool_prepared_statements: true,
                max_pool_prepared_statement_per_connection_size: 50,
                min_evictable_idle_time_millis: 600_000,
                max_evictable_idle_time_millis: 2_520_000,
                default_auto_commit: true,
                query_timeout: 30,
                login_timeout: 60,
                execute_count,
                execute_query_count: execute_count * 3 / 4,
                execute_update_count: execute_count / 5,
                execute_batch_count: execute_count / 50,
                error_count: execute_count / 400,
                commit_count: execute_count / 2,
                rollback_count: execute_count / 800,
                start_transaction_count: execute_count / 2,
                logic_connect_count: execute_count,
                logic_close_count: execute_count - active,
                logic_connect_error_count: execute_count / 400,
                driver_connect_count: execute_count / 100 + 20,
                driver_close_count: execute_count / 100 + 12,
                driver_connect_error_count: 2,
                ps_cache_access_count: execute_count / 3,
                ps_cache_hit_count: execute_count / 4,
                ps_cache_miss_count: execute_count / 12,
                clob_open_count: execute_count / 200,
                blob_open_count: execute_count / 300,
                keep_alive_check_count: execute_count / 60,
                recycle_error_count: 1,
                prepared_statement_open_count: execute_count / 10,
                prepared_statement_closed_count: execute_count / 11,
                filter_class_names: Some(vec!["stat".into(), "wall".into(), "slf4j".into()]),
                transaction_histogram: Some(vec![
                    execute_count / 2,
                    execute_count / 4,
                    execute_count / 8,
                    execute_count / 32,
                    execute_count / 128,
                    40,
                    8,
                    2,
                ]),
                connection_hold_time_histogram: Some(vec![
                    execute_count / 3,
                    execute_count / 3,
                    execute_count / 6,
                    execute_count / 24,
                    execute_count / 96,
                    60,
                    15,
                    4,
                ]),
                ..Default::default()
            },
            sql_stats: sample_sql_stats(identity),
            wall: sample_wall(identity),
            last_updated_ms: 1_700_000_000_000,
            sequence: 1,
        };
        repository.upsert_datasource(
            DataSourceId {
                service_identity,
                identity,
            },
            entry,
        );
    }

    let app = druid_admin::standalone_router(repository);
    let listener = tokio::net::TcpListener::bind(LISTEN).await.expect("bind");
    println!("Druid Admin console (demo data):");
    println!("    http://{LISTEN}/druid/");
    println!();
    println!("    示例数据：2 个数据源 / 8 条 SQL 统计 / Wall 汇总");
    println!();
    println!("Ctrl+C 退出。");
    axum::serve(listener, app).await.expect("serve");
}

fn sample_sql_stats(identity: i64) -> HashMap<i64, SqlListContent> {
    let rows = [
        ("select * from orders where id = ?", 4820, 37, 4820),
        (
            "select * from order_items where order_id = ?",
            3110,
            21,
            3110,
        ),
        ("update orders set status = ? where id = ?", 940, 8, 940),
        (
            "insert into audit_log (actor, action) values (?, ?)",
            612,
            5,
            612,
        ),
        // 慢 SQL：报表大查询，多数执行落在秒级以上桶。
        (
            "select * from orders o join order_items i on o.id = i.order_id where o.created_at > ? order by o.created_at desc",
            320,
            12_500,
            4_800,
        ),
        (
            "select count(*) from audit_log where action = ? and created_at between ? and ?",
            88,
            3_800,
            1,
        ),
    ];
    let offset = identity * 100;
    rows.iter()
        .enumerate()
        .map(|(index, (sql, executes, millis, rows))| {
            let id = offset + index as i64;
            (
                id,
                SqlListContent {
                    id,
                    hash: id,
                    sql: Some((*sql).to_owned()),
                    execute_count: *executes,
                    error_count: if index == 2 { 3 } else { 0 },
                    // 慢 SQL 行的总耗时按「次数 × 平均耗时」放大数据量。
                    total_time: if index >= 4 {
                        *millis * *executes / 5
                    } else {
                        *millis
                    },
                    max_timespan: millis * 3,
                    fetch_row_count: *rows,
                    fetch_row_count_max: rows * 2,
                    effected_row_count: *rows / 2,
                    effected_row_count_max: rows / 2,
                    concurrent_max: 24,
                    running_count: index as i64 + 1,
                    in_transaction_count: executes / 3,
                    db_type: Some("postgresql".to_owned()),
                    // 慢 SQL 行的执行耗时直方图把质量压到秒级以上桶。
                    histogram: if index >= 4 {
                        Some(vec![
                            executes / 10,
                            executes / 8,
                            executes / 5,
                            executes / 4,
                            executes / 4,
                            executes / 10,
                            executes / 40,
                            executes / 100,
                        ])
                    } else {
                        Some(vec![
                            executes / 2,
                            executes / 3,
                            executes / 8,
                            executes / 40,
                            20,
                            6,
                            2,
                            1,
                        ])
                    },
                    fetch_row_count_histogram: Some(vec![
                        executes / 3,
                        executes / 3,
                        executes / 5,
                        executes / 20,
                        15,
                        5,
                    ]),
                    effected_row_count_histogram: Some(vec![
                        executes / 2,
                        executes / 4,
                        executes / 10,
                        executes / 50,
                        12,
                        3,
                    ]),
                    execute_and_result_hold_time_histogram: Some(vec![
                        executes / 2,
                        executes / 4,
                        executes / 10,
                        executes / 60,
                        25,
                        8,
                        2,
                        1,
                    ]),
                    last_slow_parameters: if index >= 4 {
                        json!("created_at=2026-10-01..2026-11-01")
                    } else if index == 1 {
                        json!("id=88213")
                    } else {
                        Value::Null
                    },
                    last_error_message: if index == 2 {
                        json!("duplicate key value violates unique constraint \"orders_pkey\"")
                    } else {
                        Value::Null
                    },
                    last_error_class: if index == 2 {
                        json!("java.sql.SQLIntegrityConstraintViolationException")
                    } else {
                        Value::Null
                    },
                    last_error_time: if index == 2 {
                        json!("2026-11-01 09:42:11")
                    } else {
                        Value::Null
                    },
                    ..Default::default()
                },
            )
        })
        .collect()
}

fn sample_wall(identity: i64) -> WallResult {
    let mut wall = WallResult::default();
    wall.content.check_count = 20_480 * identity;
    wall.content.hard_check_count = 19_200 * identity;
    wall.content.violation_count = 3 * identity;
    wall
}
