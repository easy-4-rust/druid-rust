//! 旧 Druid 公共导入路径，直接指向唯一 RDBC CommonDataSource 合同。
//!
//! 对应 Java: `javax.sql.CommonDataSource`。登录超时和日志的实际行为由标准
//! `druid-rdbc` trait 定义；这里不建立第二套方法或默认实现。

pub use druid_rdbc::data_source::{CommonDataSource, RdbcLogWriter};
