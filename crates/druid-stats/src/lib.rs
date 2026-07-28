//! druid-stats — SQL 合并统计、百分位直方图、Prometheus 导出。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.stat` 包。

pub mod collector;
pub mod merge;
pub mod stat_filter;

pub use collector::StatsCollector;
pub use merge::{fingerprint, parameterize, MergedSqlStat, SqlMerger};
pub use stat_filter::StatFilter;
