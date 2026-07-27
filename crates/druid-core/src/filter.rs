//! 对应 Java 类：com.alibaba.druid.filter.Filter
//!
//! Filter trait 定义，拆分为 BeforeFilter + AfterFilter。

use crate::error::DruidError;
use crate::value::Value;
use std::time::{Duration, Instant};

/// SQL 执行上下文。
#[derive(Debug)]
pub struct ExecContext<'a> {
    pub sql: &'a str,
    pub params: &'a [Value],
    pub data_source: &'a str,
    pub start: Instant,
    pub fingerprint: Option<u64>,
}

/// 前置 Filter trait。任一返回 Err 则短路。
#[async_trait::async_trait]
pub trait BeforeFilter: Send + Sync {
    fn name(&self) -> &str;
    async fn before(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError>;
}

/// 后置 Filter trait。即使 SQL 执行失败也会调用。
#[async_trait::async_trait]
pub trait AfterFilter: Send + Sync {
    fn name(&self) -> &str;
    async fn after(&self, ctx: &ExecContext<'_>, result: &Result<crate::connection::ExecResult, DruidError>, elapsed: Duration);
}
