//! 与驱动无关的池治理策略（P3.1）。
//!
//! 这里只放**纯函数/纯策略**：不依赖 Tokio、Axum、Tonic 或任何具体驱动。
//! `druid` 运行时负责装配与调用；对应 Java 逻辑集中在
//! `com.alibaba.druid.pool.DruidDataSource`。

pub mod hashing;
pub mod shrink_policy;
pub mod two_phase;
pub mod wall;
