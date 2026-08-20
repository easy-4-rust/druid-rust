use super::{RdbcSqlStatValue, RdbcStatManager};
use crate::core::DruidError;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_SQL_STAT_ID: AtomicU64 = AtomicU64::new(0);

/// 单条参数化 SQL 的并发运行统计。
///
/// 对应 Java：`com.alibaba.druid.stat.RdbcSqlStat`。
#[derive(Debug)]
pub struct RdbcSqlStat {
    pub id: u64,
    pub sql: String,
    pub fingerprint: u64,
    pub execute_count: AtomicU64,
    pub total_time_ns: AtomicU64,
    pub max_time_ns: AtomicU64,
    execute_last_start_time_millis: AtomicU64,
    max_time_occurrence_millis: AtomicU64,
    execute_time_histogram: [AtomicU64; 8],
    pub error_count: AtomicU64,
    pub execute_batch_size_total: AtomicU64,
    pub execute_batch_size_max: AtomicU64,
    pub update_count: AtomicU64,
    pub update_count_max: AtomicU64,
    update_count_histogram: [AtomicU64; 6],
    pub fetch_row_count: AtomicU64,
    pub fetch_row_count_max: AtomicU64,
    fetch_row_count_histogram: [AtomicU64; 6],
    pub running_count: AtomicU64,
    pub concurrent_max: AtomicU64,
    pub in_transaction_count: AtomicU64,
    pub result_set_hold_time_ns: AtomicU64,
    pub execute_and_result_set_hold_time_ns: AtomicU64,
    execute_and_result_hold_time_histogram: [AtomicU64; 8],
    last_slow_parameters: RwLock<Option<String>>,
    last_error_message: RwLock<Option<String>>,
    last_error_class: RwLock<Option<String>>,
    last_error_time_millis: AtomicU64,
    name: RwLock<Option<String>>,
    file: RwLock<Option<String>>,
    db_type: RwLock<Option<String>>,
    read_string_length: AtomicU64,
    read_bytes_length: AtomicU64,
    input_stream_open_count: AtomicU64,
    reader_open_count: AtomicU64,
    clob_open_count: AtomicU64,
    blob_open_count: AtomicU64,
}

impl RdbcSqlStat {
    /// 返回当前执行上下文 SQL 名称。
    #[must_use]
    pub fn context_sql_name() -> Option<String> {
        RdbcStatManager::global()
            .stat_context()
            .and_then(|context| context.name().map(str::to_owned))
    }

    /// 设置当前执行上下文 SQL 名称，必要时创建上下文。
    pub fn set_context_sql_name(value: Option<String>) {
        let manager = RdbcStatManager::global();
        let mut context = manager
            .stat_context()
            .unwrap_or_else(|| manager.create_stat_context());
        context.set_name(value);
        manager.set_stat_context(Some(context));
    }

    /// 返回当前执行上下文 SQL 文件。
    #[must_use]
    pub fn context_sql_file() -> Option<String> {
        RdbcStatManager::global()
            .stat_context()
            .and_then(|context| context.file().map(str::to_owned))
    }

    /// 设置当前执行上下文 SQL 文件，必要时创建上下文。
    pub fn set_context_sql_file(value: Option<String>) {
        let manager = RdbcStatManager::global();
        let mut context = manager
            .stat_context()
            .unwrap_or_else(|| manager.create_stat_context());
        context.set_file(value);
        manager.set_stat_context(Some(context));
    }

    /// 设置当前执行上下文替代 SQL。
    pub fn set_context_sql(value: Option<String>) {
        let manager = RdbcStatManager::global();
        let mut context = manager
            .stat_context()
            .unwrap_or_else(|| manager.create_stat_context());
        context.set_sql(value);
        manager.set_stat_context(Some(context));
    }

    /// 创建 SQL 统计对象。
    #[must_use]
    pub fn new(sql: String, fingerprint: u64) -> Self {
        Self {
            id: NEXT_SQL_STAT_ID
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1),
            sql,
            fingerprint,
            execute_count: AtomicU64::new(0),
            total_time_ns: AtomicU64::new(0),
            max_time_ns: AtomicU64::new(0),
            execute_last_start_time_millis: AtomicU64::new(0),
            max_time_occurrence_millis: AtomicU64::new(0),
            execute_time_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            error_count: AtomicU64::new(0),
            execute_batch_size_total: AtomicU64::new(0),
            execute_batch_size_max: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
            update_count_max: AtomicU64::new(0),
            update_count_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            fetch_row_count: AtomicU64::new(0),
            fetch_row_count_max: AtomicU64::new(0),
            fetch_row_count_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            running_count: AtomicU64::new(0),
            concurrent_max: AtomicU64::new(0),
            in_transaction_count: AtomicU64::new(0),
            result_set_hold_time_ns: AtomicU64::new(0),
            execute_and_result_set_hold_time_ns: AtomicU64::new(0),
            execute_and_result_hold_time_histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            last_slow_parameters: RwLock::new(None),
            last_error_message: RwLock::new(None),
            last_error_class: RwLock::new(None),
            last_error_time_millis: AtomicU64::new(0),
            name: RwLock::new(None),
            file: RwLock::new(None),
            db_type: RwLock::new(None),
            read_string_length: AtomicU64::new(0),
            read_bytes_length: AtomicU64::new(0),
            input_stream_open_count: AtomicU64::new(0),
            reader_open_count: AtomicU64::new(0),
            clob_open_count: AtomicU64::new(0),
            blob_open_count: AtomicU64::new(0),
        }
    }

    /// 记录一次完成的 SQL 执行。
    pub fn record(&self, elapsed: Duration, ok: bool) {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.total_time_ns.fetch_add(nanos, Ordering::Relaxed);
        let previous_max = self.max_time_ns.fetch_max(nanos, Ordering::AcqRel);
        if nanos > previous_max {
            // Java 同样用“性能换取一致性”的近似时点：只有成功提升最大值的
            // 执行写入当前墙钟时间。
            self.max_time_occurrence_millis
                .store(epoch_millis(), Ordering::Release);
        }
        self.execute_time_histogram[time_bucket(elapsed)].fetch_add(1, Ordering::Relaxed);
        if ok {
            // Java `ExecuteCount` 实际来自 executeSuccessCount。
            self.execute_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 返回执行次数。
    #[must_use]
    pub fn execute_count(&self) -> u64 {
        self.execute_count.load(Ordering::Relaxed)
    }

    /// 返回总耗时毫秒。
    #[must_use]
    pub fn total_time_ms(&self) -> f64 {
        self.total_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// 返回最大耗时毫秒。
    #[must_use]
    pub fn max_time_ms(&self) -> f64 {
        self.max_time_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// 返回错误次数。
    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// 标记一次 SQL 开始运行并更新并发峰值。
    ///
    /// 对应 Java：`RdbcSqlStat#incrementRunningCount()`。
    pub fn increment_running_count(&self) {
        self.execute_last_start_time_millis
            .store(epoch_millis(), Ordering::Release);
        let running = self.running_count.fetch_add(1, Ordering::AcqRel) + 1;
        self.concurrent_max.fetch_max(running, Ordering::AcqRel);
    }

    /// 更新 SQL 管理快照的来源身份。
    ///
    /// 对应 Java：`RdbcDataSourceStat#createSqlStat` 及
    /// `StatFilter#internalBeforeStatementExecute`。
    pub fn set_management_identity(
        &self,
        name: Option<&str>,
        file: Option<&str>,
        db_type: Option<&str>,
    ) {
        *self.name.write() = name.map(str::to_owned);
        *self.file.write() = file.map(str::to_owned);
        *self.db_type.write() = db_type.map(str::to_owned);
    }

    /// 标记一次 SQL 执行离开运行态。
    ///
    /// 正常 after、物理错误及 before 短路都必须调用；饱和减法防止错误展开
    /// 在容量淘汰重建的极端竞态下产生无符号下溢。
    pub fn decrement_running_count(&self) {
        let _ = self
            .running_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |running| {
                Some(running.saturating_sub(1))
            });
    }

    /// 记录一次发生在显式事务中的 SQL 执行。
    pub fn increment_in_transaction_count(&self) {
        self.in_transaction_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 返回执行耗时八档直方图。
    #[must_use]
    pub fn execute_time_histogram_values(&self) -> [u64; 8] {
        std::array::from_fn(|index| self.execute_time_histogram[index].load(Ordering::Acquire))
    }

    /// 记录没有 ResultSet 首结果的执行耗时。
    pub fn record_execute_and_result_hold_time(&self, elapsed: Duration) {
        self.execute_and_result_hold_time_histogram[time_bucket(elapsed)]
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 在 ResultSet 首次关闭时记录持有时长及“执行 + 持有”总时长。
    ///
    /// 对应 Java：`RdbcSqlStat#addResultSetHoldTimeNano(long,long)`，包括其
    /// 将一次 ResultSet 关闭计入 update `<1` 桶的历史行为。
    pub fn add_result_set_hold_time(
        &self,
        statement_execute_elapsed: Duration,
        result_set_hold_elapsed: Duration,
    ) {
        let execute_nanos = u64::try_from(statement_execute_elapsed.as_nanos()).unwrap_or(u64::MAX);
        let hold_nanos = u64::try_from(result_set_hold_elapsed.as_nanos()).unwrap_or(u64::MAX);
        let combined_nanos = execute_nanos.saturating_add(hold_nanos);
        self.result_set_hold_time_ns
            .fetch_add(hold_nanos, Ordering::Relaxed);
        self.execute_and_result_set_hold_time_ns
            .fetch_add(combined_nanos, Ordering::Relaxed);
        self.execute_and_result_hold_time_histogram
            [time_bucket(statement_execute_elapsed.saturating_add(result_set_hold_elapsed))]
        .fetch_add(1, Ordering::Relaxed);
        self.update_count_histogram[0].fetch_add(1, Ordering::Relaxed);
    }

    /// 返回“执行 + 结果持有”耗时八档直方图。
    #[must_use]
    pub fn execute_and_result_hold_time_histogram_values(&self) -> [u64; 8] {
        std::array::from_fn(|index| {
            self.execute_and_result_hold_time_histogram[index].load(Ordering::Acquire)
        })
    }

    /// 保存最近一次慢 SQL 参数 JSON。
    pub fn set_last_slow_parameters(&self, parameters: Option<String>) {
        *self.last_slow_parameters.write() = parameters;
    }

    /// 返回最近一次慢 SQL 参数 JSON。
    #[must_use]
    pub fn last_slow_parameters(&self) -> Option<String> {
        self.last_slow_parameters.read().clone()
    }

    /// 保存最近一次 SQL 错误的公开诊断字段。
    pub fn record_error_detail(&self, error: &DruidError) {
        *self.last_error_message.write() = Some(error.to_string());
        *self.last_error_class.write() = Some(error.class_name().to_owned());
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        self.last_error_time_millis.store(millis, Ordering::Release);
    }

    /// 累加 ResultSet `getString` 读取长度。
    pub fn add_read_string_length(&self, length: u64) {
        self.read_string_length.fetch_add(length, Ordering::Relaxed);
    }

    /// 累加 ResultSet `getBytes` 读取长度。
    pub fn add_read_bytes_length(&self, length: u64) {
        self.read_bytes_length.fetch_add(length, Ordering::Relaxed);
    }

    /// 累加 ResultSet 打开的 InputStream 数量。
    pub fn add_input_stream_open_count(&self, count: u64) {
        self.input_stream_open_count
            .fetch_add(count, Ordering::Relaxed);
    }

    /// 累加 ResultSet 打开的 Reader 数量。
    pub fn add_reader_open_count(&self, count: u64) {
        self.reader_open_count.fetch_add(count, Ordering::Relaxed);
    }

    /// 记录成功打开一个非空 Clob/NClob。
    ///
    /// 对应 Java：`RdbcSqlStat#incrementClobOpenCount()`。
    pub fn increment_clob_open_count(&self) {
        self.clob_open_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录成功打开一个非空 Blob。
    ///
    /// 对应 Java：`RdbcSqlStat#incrementBlobOpenCount()`。
    pub fn increment_blob_open_count(&self) {
        self.blob_open_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 累加一次 batch 的元素数并更新历史峰值。
    ///
    /// 对应 Java：`RdbcSqlStat#addExecuteBatchCount(long)`。
    pub fn add_execute_batch_count(&self, batch_size: usize) {
        let batch_size = u64::try_from(batch_size).unwrap_or(u64::MAX);
        self.execute_batch_size_total
            .fetch_add(batch_size, Ordering::Relaxed);
        self.execute_batch_size_max
            .fetch_max(batch_size, Ordering::Relaxed);
    }

    /// 累加一次更新影响行数并更新峰值和六档直方图。
    ///
    /// 对应 Java：`RdbcSqlStat#addUpdateCount(int)`。负值和零不进入总数，
    /// 但仍进入 `<1` 档；峰值比较保留原始 RDBC 结果。
    pub fn add_update_count(&self, delta: i32) {
        if delta > 0 {
            self.update_count
                .fetch_add(u64::try_from(delta).unwrap_or_default(), Ordering::Relaxed);
        }
        if delta > 0 {
            self.update_count_max
                .fetch_max(u64::try_from(delta).unwrap_or_default(), Ordering::Relaxed);
        }
        self.update_count_histogram[row_count_bucket(i64::from(delta))]
            .fetch_add(1, Ordering::Relaxed);
    }

    /// 累加一次结果集抓取行数并更新峰值和六档直方图。
    ///
    /// 对应 Java：`RdbcSqlStat#addFetchRowCount(long)`。
    pub fn add_fetch_row_count(&self, delta: u64) {
        self.fetch_row_count.fetch_add(delta, Ordering::Relaxed);
        self.fetch_row_count_max.fetch_max(delta, Ordering::Relaxed);
        let bucket = row_count_bucket(i64::try_from(delta).unwrap_or(i64::MAX));
        self.fetch_row_count_histogram[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// 返回更新影响行数六档直方图。
    #[must_use]
    pub fn update_count_histogram_values(&self) -> [u64; 6] {
        std::array::from_fn(|index| self.update_count_histogram[index].load(Ordering::Acquire))
    }

    /// 返回结果集抓取行数六档直方图。
    #[must_use]
    pub fn fetch_row_count_histogram_values(&self) -> [u64; 6] {
        std::array::from_fn(|index| self.fetch_row_count_histogram[index].load(Ordering::Acquire))
    }

    /// 返回不可变管理快照。
    #[must_use]
    pub fn stat_value(&self) -> RdbcSqlStatValue {
        RdbcSqlStatValue {
            id: self.id,
            sql: self.sql.clone(),
            hash: java_fnv1a_64(&self.sql),
            execute_count: self.execute_count(),
            total_time_millis: self.total_time_ms() as u64,
            last_time_millis: match self.execute_last_start_time_millis.load(Ordering::Acquire) {
                0 => None,
                millis => Some(millis),
            },
            max_timespan_millis: self.max_time_ms() as u64,
            max_timespan_occur_time_millis: match self
                .max_time_occurrence_millis
                .load(Ordering::Acquire)
            {
                0 => None,
                millis => Some(millis),
            },
            execute_time_histogram: self.execute_time_histogram_values(),
            error_count: self.error_count(),
            execute_batch_size_total: self.execute_batch_size_total.load(Ordering::Acquire),
            execute_batch_size_max: self.execute_batch_size_max.load(Ordering::Acquire),
            update_count: self.update_count.load(Ordering::Acquire),
            update_count_max: self.update_count_max.load(Ordering::Acquire),
            update_count_histogram: self.update_count_histogram_values(),
            fetch_row_count: self.fetch_row_count.load(Ordering::Acquire),
            fetch_row_count_max: self.fetch_row_count_max.load(Ordering::Acquire),
            fetch_row_count_histogram: self.fetch_row_count_histogram_values(),
            running_count: self.running_count.load(Ordering::Acquire),
            concurrent_max: self.concurrent_max.load(Ordering::Acquire),
            in_transaction_count: self.in_transaction_count.load(Ordering::Acquire),
            result_set_hold_time_millis: self.result_set_hold_time_ns.load(Ordering::Acquire)
                / 1_000_000,
            execute_and_result_set_hold_time_millis: self
                .execute_and_result_set_hold_time_ns
                .load(Ordering::Acquire)
                / 1_000_000,
            execute_and_result_hold_time_histogram: self
                .execute_and_result_hold_time_histogram_values(),
            last_slow_parameters: self.last_slow_parameters(),
            last_error_message: self.last_error_message.read().clone(),
            last_error_class: self.last_error_class.read().clone(),
            last_error: self.last_error_message.read().as_ref().map(|message| {
                serde_json::json!({
                    "class": self.last_error_class.read().clone(),
                    "message": message,
                    "stackTrace": serde_json::Value::Null,
                })
            }),
            // Rust 错误默认不捕获 JVM 风格 stack trace；字段保留为 null。
            last_error_stack_trace: None,
            last_error_time_millis: match self.last_error_time_millis.load(Ordering::Acquire) {
                0 => None,
                millis => Some(millis),
            },
            read_string_length: self.read_string_length.load(Ordering::Acquire),
            read_bytes_length: self.read_bytes_length.load(Ordering::Acquire),
            input_stream_open_count: self.input_stream_open_count.load(Ordering::Acquire),
            reader_open_count: self.reader_open_count.load(Ordering::Acquire),
            clob_open_count: self.clob_open_count.load(Ordering::Acquire),
            blob_open_count: self.blob_open_count.load(Ordering::Acquire),
            data_source: None,
            name: self.name.read().clone(),
            file: self.file.read().clone(),
            db_type: self.db_type.read().clone(),
            url: None,
        }
    }

    /// 清零区间统计但保留 SQL 身份和当前 runningCount。
    ///
    /// 对应 Java：`RdbcSqlStat#reset()`；运行中的执行不能被重置为零。
    pub fn reset(&self) {
        self.execute_count.store(0, Ordering::Release);
        self.total_time_ns.store(0, Ordering::Release);
        self.max_time_ns.store(0, Ordering::Release);
        self.execute_last_start_time_millis
            .store(0, Ordering::Release);
        self.max_time_occurrence_millis.store(0, Ordering::Release);
        self.error_count.store(0, Ordering::Release);
        self.execute_batch_size_total.store(0, Ordering::Release);
        self.execute_batch_size_max.store(0, Ordering::Release);
        self.update_count.store(0, Ordering::Release);
        self.update_count_max.store(0, Ordering::Release);
        self.fetch_row_count.store(0, Ordering::Release);
        self.fetch_row_count_max.store(0, Ordering::Release);
        self.concurrent_max.store(0, Ordering::Release);
        self.in_transaction_count.store(0, Ordering::Release);
        self.result_set_hold_time_ns.store(0, Ordering::Release);
        self.execute_and_result_set_hold_time_ns
            .store(0, Ordering::Release);
        self.set_last_slow_parameters(None);
        *self.last_error_message.write() = None;
        *self.last_error_class.write() = None;
        self.last_error_time_millis.store(0, Ordering::Release);
        self.read_string_length.store(0, Ordering::Release);
        self.read_bytes_length.store(0, Ordering::Release);
        self.input_stream_open_count.store(0, Ordering::Release);
        self.reader_open_count.store(0, Ordering::Release);
        self.clob_open_count.store(0, Ordering::Release);
        self.blob_open_count.store(0, Ordering::Release);
        reset_histogram(&self.execute_time_histogram);
        reset_histogram(&self.update_count_histogram);
        reset_histogram(&self.fetch_row_count_histogram);
        reset_histogram(&self.execute_and_result_hold_time_histogram);
    }
}

fn reset_histogram<const N: usize>(histogram: &[AtomicU64; N]) {
    for bucket in histogram {
        bucket.store(0, Ordering::Release);
    }
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// 按 Java `FnvHash#fnv1a_64(String)` 对 UTF-16 code unit 计算有符号 long。
fn java_fnv1a_64(value: &str) -> i64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for code_unit in value.encode_utf16() {
        hash ^= u64::from(code_unit);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash as i64
}

fn time_bucket(elapsed: Duration) -> usize {
    match elapsed.as_millis() {
        ..1 => 0,
        1..10 => 1,
        10..100 => 2,
        100..1_000 => 3,
        1_000..10_000 => 4,
        10_000..100_000 => 5,
        100_000..1_000_000 => 6,
        _ => 7,
    }
}

fn row_count_bucket(value: i64) -> usize {
    match value {
        ..1 => 0,
        1..10 => 1,
        10..100 => 2,
        100..1_000 => 3,
        1_000..10_000 => 4,
        _ => 5,
    }
}
