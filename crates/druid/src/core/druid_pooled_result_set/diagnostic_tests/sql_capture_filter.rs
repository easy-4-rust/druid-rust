//! 观察真实结果集打开上下文的 SQL，不自行构造 SQL 状态。

use crate::core::standard_result_set_open_context::StandardResultSetOpenContext;
use crate::core::{DruidError, ResultSetFilter};
use std::sync::Mutex;

/// 捕获生产上下文 SQL 的测试 Filter。对应 Java: ResultSetProxy#getSql。
#[derive(Default)]
pub(super) struct SqlCaptureFilter {
    sql: Mutex<Option<String>>,
}

impl SqlCaptureFilter {
    /// 无参数；返回生产 getter 捕获的 SQL 副本或 None。
    pub(super) fn sql(&self) -> Option<String> {
        self.sql.lock().unwrap().clone()
    }
}

impl ResultSetFilter for SqlCaptureFilter {
    /// 参数为实际打开上下文；读取其 SQL，返回成功观察结果。
    fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        *self.sql.lock().unwrap() = context.filter_context().sql().map(str::to_owned);
        Ok(())
    }
}
