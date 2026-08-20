use super::DataSourceIdentity;

/// Point-in-time telemetry snapshot for a single datasource.
///
/// Contains the datasource identity, pool state, SQL statistics, optional
/// Wall snapshot, and the sampling timestamp. All fields are owned so the
/// snapshot can be sent across threads without holding any locks.
#[derive(Debug, Clone)]
pub struct DruidTelemetrySnapshot {
    /// Datasource identity at snapshot time.
    pub identity: DataSourceIdentity,
    /// Connection pool state.
    pub pool_snapshot: PoolSnapshot,
    /// Per-SQL statistics (one entry per normalized SQL).
    pub sql_stats: Vec<SqlStatSnapshot>,
    /// Optional Wall (firewall) statistics.
    pub wall_snapshot: Option<WallSnapshot>,
    /// Sampling timestamp in milliseconds since Unix epoch.
    pub sampling_time_millis: u64,
}

/// Connection pool state at snapshot time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSnapshot {
    /// Number of active (borrowed) connections.
    pub active_count: u32,
    /// Number of idle connections in the pool.
    pub idle_count: u32,
    /// Maximum number of active connections allowed.
    pub max_active: u32,
    /// Maximum number of idle connections allowed.
    pub max_idle: u32,
    /// Number of threads waiting for a connection.
    pub waiting_count: u32,
}

/// Per-SQL statistics snapshot.
#[derive(Debug, Clone)]
pub struct SqlStatSnapshot {
    /// Normalized SQL fingerprint.
    pub fingerprint: String,
    /// Total execution count.
    pub exec_count: u64,
    /// Total execution time in milliseconds.
    pub exec_time_millis: u64,
    /// Number of rows returned/fetched.
    pub fetch_row_count: u64,
    /// Number of updates affected.
    pub update_count: u64,
}

/// Wall (firewall) statistics snapshot.
#[derive(Debug, Clone)]
pub struct WallSnapshot {
    /// Number of SQL statements checked by the Wall.
    pub check_count: u64,
    /// Number of SQL statements denied by the Wall.
    pub deny_count: u64,
    /// Number of violations detected.
    pub violation_count: u64,
}
