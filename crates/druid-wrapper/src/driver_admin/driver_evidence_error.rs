/// 数据库产品合同证据聚合错误。
#[derive(Debug, thiserror::Error)]
pub enum DriverEvidenceError {
    #[error("evidence filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("evidence JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evidence contract is incomplete: {0}")]
    Incomplete(String),
}
