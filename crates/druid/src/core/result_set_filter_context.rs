//! `ResultSet Filter` 调用上下文。
//!
//! 对应 Java 平台对象：
//! `com.alibaba.druid.proxy.rdbc.ResultSetProxyImpl` 中由 Filter 使用的状态。

use parking_lot::RwLock;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// 在一条 `ResultSet Filter` 调用链中共享的可观测状态。
///
/// 该对象保留 Java `constructNano`、`fetchRowCount` 与 `closeCount` 的更新时机，
/// 但不持有物理结果集，避免与池化 Statement 形成所有权环。
#[derive(Debug)]
pub struct ResultSetFilterContext {
    connection_id: u64,
    statement_id: u64,
    result_set_id: u64,
    construct_time: OnceLock<Instant>,
    sql: OnceLock<String>,
    statement_execute_elapsed: OnceLock<Duration>,
    fetch_row_count: AtomicI32,
    close_count: AtomicU64,
    read_string_length: AtomicU64,
    read_bytes_length: AtomicU64,
    open_input_stream_count: AtomicU64,
    open_reader_count: AtomicU64,
    tenant_columns: RwLock<Vec<usize>>,
}

impl ResultSetFilterContext {
    /// 创建尚未设置构造时刻、抓取数和关闭数均为零的上下文。
    pub fn new() -> Self {
        Self {
            connection_id: 0,
            statement_id: 0,
            result_set_id: 0,
            construct_time: OnceLock::new(),
            sql: OnceLock::new(),
            statement_execute_elapsed: OnceLock::new(),
            fetch_row_count: AtomicI32::new(0),
            close_count: AtomicU64::new(0),
            read_string_length: AtomicU64::new(0),
            read_bytes_length: AtomicU64::new(0),
            open_input_stream_count: AtomicU64::new(0),
            open_reader_count: AtomicU64::new(0),
            tenant_columns: RwLock::new(Vec::new()),
        }
    }

    /// 创建并绑定产生该 ResultSet 的 SQL。
    #[must_use]
    pub fn with_sql_and_execute_elapsed(
        sql: Option<String>,
        statement_execute_elapsed: Option<Duration>,
    ) -> Self {
        let context = Self::new();
        if let Some(sql) = sql {
            let _ = context.sql.set(sql);
        }
        if let Some(elapsed) = statement_execute_elapsed {
            let _ = context.statement_execute_elapsed.set(elapsed);
        }
        context
    }

    /// 创建并绑定 Druid Proxy 身份、SQL 与 Statement 执行耗时。
    ///
    /// 对应 Java：`ResultSetProxyImpl(StatementProxy, ResultSet, long, String)`。
    #[must_use]
    pub fn with_identity_sql_and_execute_elapsed(
        connection_id: u64,
        statement_id: u64,
        result_set_id: u64,
        sql: Option<String>,
        statement_execute_elapsed: Option<Duration>,
    ) -> Self {
        let mut context = Self::with_sql_and_execute_elapsed(sql, statement_execute_elapsed);
        context.connection_id = connection_id;
        context.statement_id = statement_id;
        context.result_set_id = result_set_id;
        context
    }

    /// 返回创建本结果集的 Druid 连接 ID。
    #[must_use]
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 返回创建本结果集的 Statement ID。
    #[must_use]
    pub const fn statement_id(&self) -> u64 {
        self.statement_id
    }

    /// 返回本 ResultSet ID。
    #[must_use]
    pub const fn result_set_id(&self) -> u64 {
        self.result_set_id
    }

    /// 返回产生该 ResultSet 的 SQL。
    #[must_use]
    pub fn sql(&self) -> Option<&str> {
        self.sql.get().map(String::as_str)
    }

    /// 返回创建该 ResultSet 的 Statement 执行耗时。
    #[must_use]
    pub fn statement_execute_elapsed(&self) -> Option<Duration> {
        self.statement_execute_elapsed.get().copied()
    }

    /// 仅在尚未设置时记录构造时刻。
    ///
    /// 对应 Java：`ResultSetProxyImpl#setConstructNano()`。
    pub fn set_construct_time(&self) {
        let _ = self.construct_time.set(Instant::now());
    }

    /// 返回从构造时刻到当前的耗时；尚未设置时返回 `None`。
    pub fn elapsed(&self) -> Option<Duration> {
        self.construct_time.get().map(Instant::elapsed)
    }

    /// 记录成功抓取的历史峰值行号。
    pub fn record_fetch_row_count(&self, fetch_row_count: i32) {
        self.fetch_row_count
            .fetch_max(fetch_row_count, Ordering::AcqRel);
    }

    /// 返回成功抓取的历史峰值行号。
    pub fn fetch_row_count(&self) -> i32 {
        self.fetch_row_count.load(Ordering::Acquire)
    }

    /// 在整条物理 close 链成功后增加关闭次数。
    ///
    /// 对应 Java：`ResultSetProxyImpl#close()` 在
    /// `chain.resultSet_close(this)` 返回之后执行 `closeCount++`。
    pub fn increment_close_count(&self) {
        self.close_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回成功完成的 Filter close 链次数。
    pub fn close_count(&self) -> u64 {
        self.close_count.load(Ordering::Acquire)
    }

    /// 累加 `getString` 成功返回的 Java UTF-16 code unit 数。
    pub fn add_read_string_length(&self, value: &str) {
        self.read_string_length.fetch_add(
            u64::try_from(value.encode_utf16().count()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }

    /// 累加 `getBytes` 成功返回的字节数。
    pub fn add_read_bytes_length(&self, length: usize) {
        self.read_bytes_length
            .fetch_add(u64::try_from(length).unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    /// 记录一次成功打开的 ASCII/Binary InputStream。
    pub fn increment_open_input_stream_count(&self) {
        self.open_input_stream_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一次成功打开的 CharacterStream Reader。
    pub fn increment_open_reader_count(&self) {
        self.open_reader_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 返回读取字符串总长度。
    pub fn read_string_length(&self) -> u64 {
        self.read_string_length.load(Ordering::Acquire)
    }

    /// 返回读取字节总长度。
    pub fn read_bytes_length(&self) -> u64 {
        self.read_bytes_length.load(Ordering::Acquire)
    }

    /// 返回打开 InputStream 次数。
    pub fn open_input_stream_count(&self) -> u64 {
        self.open_input_stream_count.load(Ordering::Acquire)
    }

    /// 返回打开 Reader 次数。
    pub fn open_reader_count(&self) -> u64 {
        self.open_reader_count.load(Ordering::Acquire)
    }

    /// 替换当前 ResultSet 中需要在 `next()` 成功后检查的租户列。
    ///
    /// 对应 Java：`WallFilter.tenantColumnsLocal`。Rust 将状态绑定到 ResultSet
    /// 身份，避免异步任务跨线程时丢失或串用另一个游标的列配置。
    pub fn set_tenant_columns(&self, tenant_columns: Vec<usize>) {
        *self.tenant_columns.write() = tenant_columns;
    }

    /// 返回租户物理列的稳定快照。
    #[must_use]
    pub fn tenant_columns(&self) -> Vec<usize> {
        self.tenant_columns.read().clone()
    }
}

impl Default for ResultSetFilterContext {
    fn default() -> Self {
        Self::new()
    }
}
