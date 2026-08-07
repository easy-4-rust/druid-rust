//! Druid 管理端的 Axum 实现。
//!
//! 对应 Java 模块：`druid-admin`。本 crate 保留 Java 管理协议和 JSON
//! 字段，同时用可注入的发现与 HTTP SPI 替代 Spring Cloud/Kubernetes
//! 静态依赖。

pub mod admin_state;
pub mod config;
/// 显式 JDBC 驱动安装、内容校验和运行时诊断。
#[cfg(feature = "managed-driver-install")]
pub mod driver;
pub mod druid_admin_application;
pub mod model;
pub mod service;
pub mod servlet;
pub mod util;

pub use admin_state::AdminState;
pub use druid_admin_application::DruidAdminApplication;
pub use servlet::monitor_view_servlet::{endpoint_list, MonitorViewServlet};
pub use servlet::StatViewServlet;
