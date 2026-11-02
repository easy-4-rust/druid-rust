//! 成功提交后的短期安全数值历史；不保存 SQL 原文与连接信息。
use super::{DataSourceEntry, DataSourceId, MetricsRepository, MetricsRepositoryInner};
use serde_json::{json, Value};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAX_SAMPLES: usize = 4_096;

/// 一个已接受的来源采样，按复合身份和服务端单调时间保留。
/// Rust Admin 专属；Java 单体管理台没有对应的跨进程采样对象。
pub(super) struct SampleHistoryEntry {
    id: DataSourceId,
    pub(super) received_at: Instant,
    received_ms: u64,
    sampling_ms: i64,
    sequence: u64,
    epoch: Option<u64>,
    active: i64,
    idle: i64,
    max_active: i64,
    waiting: i64,
    execute_count: i64,
    execute_delta: Option<i64>,
    sql_total_time_ms: i64,
    fetched_rows: i64,
    affected_rows: i64,
    wall_checks: i64,
    wall_denies: i64,
    wall_violations: i64,
}

impl SampleHistoryEntry {
    fn new(
        id: &DataSourceId,
        entry: &DataSourceEntry,
        epoch: Option<u64>,
        previous: Option<&Self>,
    ) -> Self {
        let execute_count = entry
            .sql_stats
            .values()
            .fold(0_i64, |sum, stat| sum.saturating_add(stat.execute_count));
        let previous = previous.filter(|old| old.epoch == epoch && entry.sequence > old.sequence);
        let execute_delta = previous
            .and_then(|old| execute_count.checked_sub(old.execute_count))
            .filter(|delta| *delta >= 0);
        Self {
            id: id.clone(),
            received_at: Instant::now(),
            received_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            sampling_ms: entry.last_updated_ms,
            sequence: entry.sequence,
            epoch,
            active: entry.datasource.active_count,
            idle: entry.datasource.pooling_count,
            max_active: entry.datasource.max_active,
            waiting: entry.datasource.wait_thread_count,
            execute_count,
            execute_delta,
            sql_total_time_ms: entry
                .sql_stats
                .values()
                .fold(0_i64, |sum, stat| sum.saturating_add(stat.total_time)),
            fetched_rows: entry
                .sql_stats
                .values()
                .fold(0_i64, |sum, stat| sum.saturating_add(stat.fetch_row_count)),
            affected_rows: entry.sql_stats.values().fold(0_i64, |sum, stat| {
                sum.saturating_add(stat.effected_row_count)
            }),
            wall_checks: entry.wall.content.check_count,
            wall_denies: entry.wall.content.black_list_hit_count,
            wall_violations: entry.wall.content.violation_count,
        }
    }

    fn json(&self) -> Value {
        json!({
            "serviceId": self.id.service_identity.shared_service_id(),
            "serviceName": self.id.service_identity.shared_service_name(),
            "instanceId": self.id.service_identity.shared_instance_id(),
            "bootId": self.id.service_identity.shared_boot_id(),
            "datasourceId": self.id.identity,
            "receivedTimeMillis": self.received_ms, "samplingTimeMillis": self.sampling_ms,
            "sequence": self.sequence, "epoch": self.epoch,
            "activeCount": self.active, "poolingCount": self.idle,
            "maxActive": self.max_active, "waitThreadCount": self.waiting,
            "executeCount": self.execute_count, "executeCountDelta": self.execute_delta,
            "sqlTotalTimeMillis": self.sql_total_time_ms,
            "fetchRowCount": self.fetched_rows, "effectedRowCount": self.affected_rows,
            "wallCheckCount": self.wall_checks, "wallDenyCount": self.wall_denies,
            "wallViolationCount": self.wall_violations,
        })
    }
}

impl MetricsRepository {
    /// 只在最新状态成功提交后记录样本；总量固定上限，额外内存不随 SQL 原文增长。
    pub(super) fn append_sample(
        &self,
        inner: &mut MetricsRepositoryInner,
        id: &DataSourceId,
        entry: &DataSourceEntry,
        epoch: Option<u64>,
    ) {
        let previous = inner.history.iter().rev().find(|sample| sample.id == *id);
        inner
            .history
            .push_back(SampleHistoryEntry::new(id, entry, epoch, previous));
        while inner.history.len() > MAX_SAMPLES {
            inner.history.pop_front();
        }
    }

    /// 按可选服务和数据源身份读取最多 500 个最近样本。
    /// `limit` 会限制在 1–500；返回时间升序的安全数值，空变化量表示不可用。
    pub fn sample_history(
        &self,
        service_id: Option<&str>,
        datasource_id: Option<i64>,
        limit: usize,
    ) -> Vec<Value> {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        let mut result: Vec<Value> = inner
            .history
            .iter()
            .rev()
            .filter(|sample| {
                service_id
                    .is_none_or(|id| sample.id.service_identity.shared_service_id().as_ref() == id)
                    && datasource_id.is_none_or(|id| sample.id.identity == id)
            })
            .take(limit.clamp(1, 500))
            .map(SampleHistoryEntry::json)
            .collect();
        result.reverse();
        result
    }

    /// 返回短期采样的实际条数、容量和保留期限；`durable` 恒为 false，重启后清空。
    pub fn sample_history_status(&self) -> Value {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        json!({"storedSamples": inner.history.len(), "maxSamples": MAX_SAMPLES,
            "ttlMillis": self.config.entry_ttl().as_millis(), "durable": false})
    }
}
