/// Configuration validation errors for [`DruidMetricsConfig`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetricsConfigError {
    #[error("queue_capacity must be > 0")]
    InvalidQueueCapacity,
    #[error("batch_size must be > 0")]
    InvalidBatchSize,
    #[error("coalesce_window must be non-zero")]
    InvalidCoalesceWindow,
    #[error("sample_interval must be non-zero")]
    InvalidSampleInterval,
    #[error("shutdown_timeout must be non-zero")]
    InvalidShutdownTimeout,
}

/// Runtime errors for the metrics subsystem.
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("configuration error: {0}")]
    Config(#[from] MetricsConfigError),
    #[error("runtime already started")]
    AlreadyStarted,
    #[error("runtime not started")]
    NotStarted,
    #[error("sensitive field detected in payload: {field}")]
    SensitiveField { field: String },
    #[error("shutdown timed out, {unflushed} snapshots not flushed")]
    ShutdownTimeout { unflushed: usize },
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("channel send error: {0}")]
    ChannelSend(String),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
