/// Stable identity for a datasource instance.
///
/// Carries the numeric ID assigned by `DruidDataSourceStatManager::register`,
/// the human-readable name, and the optional Rust driver/Adapter name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataSourceIdentity {
    /// Numeric datasource ID (assigned at registration time).
    pub id: u64,
    /// Human-readable datasource name.
    pub name: String,
    /// Rust physical driver/Adapter name (e.g. "sqlite", "postgres").
    pub driver_name: Option<String>,
}
