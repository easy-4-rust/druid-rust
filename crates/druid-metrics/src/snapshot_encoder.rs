use druid::stats::{DruidTelemetrySnapshot, SqlStatSnapshot};

use crate::config::SqlTextPolicy;
use crate::protocol::{PoolSnapshotMsg, SnapshotEntry, SqlStatMsg};

/// 将 Druid 遥测快照编码为共享协议的强类型快照条目。
///
/// `max_sql_statements` 限制单个数据源进入协议的 SQL 统计数量；SQL
/// 快照只提供参数化指纹，不包含绑定参数或原始 SQL，因此
/// `RawWithoutParameters` 在此边界安全降级为同一参数化指纹。
#[must_use]
pub(crate) fn encode_snapshot(
    snapshot: DruidTelemetrySnapshot,
    max_sql_statements: usize,
    sql_text_policy: SqlTextPolicy,
) -> SnapshotEntry {
    let DruidTelemetrySnapshot {
        identity,
        pool_snapshot,
        sql_stats,
        wall_snapshot,
        sampling_time_millis,
    } = snapshot;

    let sql_stats = sql_stats
        .into_iter()
        .take(max_sql_statements)
        .map(|stat| encode_sql_stat(stat, sql_text_policy))
        .collect();
    let (wall_check_count, wall_deny_count, wall_violation_count) = wall_snapshot
        .map_or((0, 0, 0), |wall| {
            (wall.check_count, wall.deny_count, wall.violation_count)
        });

    SnapshotEntry {
        datasource_id: identity.id,
        datasource_name: identity.name,
        driver_name: identity.driver_name,
        pool: Some(PoolSnapshotMsg {
            active_count: pool_snapshot.active_count,
            idle_count: pool_snapshot.idle_count,
            max_active: pool_snapshot.max_active,
            max_idle: pool_snapshot.max_idle,
            waiting_count: pool_snapshot.waiting_count,
        }),
        sql_stats,
        wall_check_count,
        wall_deny_count,
        wall_violation_count,
        sampling_time_millis,
    }
}

fn encode_sql_stat(stat: SqlStatSnapshot, sql_text_policy: SqlTextPolicy) -> SqlStatMsg {
    let fingerprint = match sql_text_policy {
        SqlTextPolicy::Disabled => String::new(),
        SqlTextPolicy::FingerprintOnly | SqlTextPolicy::RawWithoutParameters => stat.fingerprint,
    };

    SqlStatMsg {
        fingerprint,
        exec_count: stat.exec_count,
        exec_time_millis: stat.exec_time_millis,
        fetch_row_count: stat.fetch_row_count,
        update_count: stat.update_count,
    }
}
