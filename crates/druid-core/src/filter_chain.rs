//! 对应 Java 类：com.alibaba.druid.filter.FilterChain + FilterChainImpl

use crate::connection::ExecResult;
use crate::error::DruidError;
use crate::filter::{AfterFilter, BeforeFilter, ExecContext};
use std::sync::Arc;
use std::time::Duration;

/// Filter 链。
pub struct FilterChain {
    before_filters: Vec<Arc<dyn BeforeFilter>>,
    after_filters: Vec<Arc<dyn AfterFilter>>,
}

impl FilterChain {
    pub fn new() -> Self { Self { before_filters: Vec::new(), after_filters: Vec::new() } }
    pub fn add_before(&mut self, filter: Arc<dyn BeforeFilter>) { self.before_filters.push(filter); }
    pub fn add_after(&mut self, filter: Arc<dyn AfterFilter>) { self.after_filters.push(filter); }
    pub fn before_count(&self) -> usize { self.before_filters.len() }
    pub fn after_count(&self) -> usize { self.after_filters.len() }

    pub async fn before_execute(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError> {
        for f in &self.before_filters { f.before(ctx).await?; }
        Ok(())
    }

    pub async fn after_execute(&self, ctx: &ExecContext<'_>, result: &Result<ExecResult, DruidError>, elapsed: Duration) {
        for f in self.after_filters.iter().rev() { f.after(ctx, result, elapsed).await; }
    }
}

impl Default for FilterChain { fn default() -> Self { Self::new() } }
