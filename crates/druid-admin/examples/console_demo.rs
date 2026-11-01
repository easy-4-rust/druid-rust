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
                pooling_count: pooling,
                max_active,
                wait_thread_count: 0,
                execute_count,
                error_count: execute_count / 400,
                commit_count: execute_count / 2,
                rollback_count: execute_count / 800,
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
                    total_time: *millis,
                    max_timespan: millis * 3,
                    fetch_row_count: *rows,
                    effected_row_count: *rows / 2,
                    db_type: Some("postgresql".to_owned()),
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
