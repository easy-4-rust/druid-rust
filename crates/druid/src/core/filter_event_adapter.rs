//! 对应 Java：`com.alibaba.druid.filter.FilterEventAdapter`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/FilterEventAdapter.java`。

use super::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, DruidError, ExecContext,
    ExecOperation, ExecResult, ExtendedFilter, PhysicalConnectionConnectFilterChain,
    PhysicalConnectionConnectResult, ResultSetFilter, ResultSetFilterContext, StatementEvent,
    Wrapper,
};
use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

/// `FilterEventAdapter` 的可覆盖事件模板。
///
/// Java 通过匿名子类覆盖 protected 方法；Rust 不使用继承，因此把同一组模板
/// 方法放在组合监听器中。方法默认不产生副作用，返回错误时中止当前调用。
#[async_trait::async_trait]
pub trait FilterEventListener: Send + Sync {
    /// 物理连接创建前事件。
    ///
    /// 对应 Java：`FilterEventAdapter#connection_connectBefore`。
    /// 返回事件处理结果；错误会中止连接创建。
    async fn connection_connect_before(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 物理连接创建成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#connection_connectAfter`。
    /// 返回事件处理结果；错误会替代本次连接创建结果。
    async fn connection_connect_after(&self, _connection_id: u64) -> Result<(), DruidError> {
        Ok(())
    }

    /// 普通 Statement 创建成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementCreateAfter`。当前 Rust Filter
    /// 事件只暴露创建种类，Statement 平台对象身份仍由后续迁移切片补齐。
    /// 返回事件处理结果；错误会使创建调用失败。
    async fn statement_create_after(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// PreparedStatement 创建成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementPrepareAfter`。
    /// 参数 `sql` 为原始预编译 SQL；返回事件处理结果。
    async fn statement_prepare_after(&self, _sql: &str) -> Result<(), DruidError> {
        Ok(())
    }

    /// CallableStatement 创建成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementPrepareCallAfter`。
    /// 参数 `sql` 为原始调用 SQL；返回事件处理结果。
    async fn statement_prepare_call_after(&self, _sql: &str) -> Result<(), DruidError> {
        Ok(())
    }

    /// generic execute 前事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteBefore`。
    /// 参数 `context` 保留 SQL 与执行入口；返回错误会阻止物理执行。
    async fn statement_execute_before(&self, _context: &ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    /// generic execute 成功后事件。
    ///
    /// `first_result` 与 JDBC `Statement#execute` 返回值一致：首结果为结果集时
    /// 为 `true`，更新计数或无结果时为 `false`。
    /// 参数 `context` 保留执行上下文；返回错误会进入 error-after。
    async fn statement_execute_after(
        &self,
        _context: &ExecContext<'_>,
        _first_result: bool,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeQuery 前事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteQueryBefore`。
    /// 参数 `context` 保留查询上下文；返回错误会阻止物理查询。
    async fn statement_execute_query_before(
        &self,
        _context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeQuery 成功后事件。
    ///
    /// Java 回调还接收 `ResultSetProxy`；Rust 的具体结果集打开事件由
    /// [`Self::result_set_open_after`] 接收，避免在异步 SQL 上下文中伪造句柄。
    /// 参数 `context` 保留查询上下文；返回错误会进入 error-after。
    async fn statement_execute_query_after(
        &self,
        _context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeUpdate 前事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteUpdateBefore`。
    /// 参数 `context` 保留更新上下文；返回错误会阻止物理更新。
    async fn statement_execute_update_before(
        &self,
        _context: &ExecContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeUpdate 成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteUpdateAfter`。
    /// 参数含执行上下文和 JDBC `int` 更新计数；返回错误会进入 error-after。
    async fn statement_execute_update_after(
        &self,
        _context: &ExecContext<'_>,
        _update_count: i32,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeBatch 前事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteBatchBefore`。
    /// 参数 `context` 保留批次输入；返回错误会阻止物理批量执行。
    async fn statement_execute_batch_before(
        &self,
        _context: &BatchExecContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// executeBatch 成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statementExecuteBatchAfter`。
    /// 参数含批次上下文和 JDBC `int[]` 计数；返回错误会进入 error-after。
    async fn statement_execute_batch_after(
        &self,
        _context: &BatchExecContext<'_>,
        _update_counts: &[i32],
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Statement/PreparedStatement 执行失败后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#statement_executeErrorAfter`。若本回调再次
    /// 返回错误，新错误与 Java error-after 抛出异常相同，会替代原始错误。
    /// 参数保留原 SQL 和原错误；返回成功表示继续传播原错误。
    async fn statement_execute_error_after(
        &self,
        _sql: &str,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 结果集代理创建成功后事件。
    ///
    /// 对应 Java：`FilterEventAdapter#resultSetOpenAfter`。
    /// 参数 `context` 标识结果集；返回错误会使打开结果集调用失败。
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl FilterEventListener for () {}

/// 把 Java `FilterEventAdapter` 的 before/after/error 模板映射到 Rust Filter 链。
///
/// Java 对象继承 `FilterAdapter` 并通过 protected 方法供子类覆写。Rust 对象
/// 持有一个 [`FilterEventListener`]，在相同成功、失败与 ResultSet 打开边界
/// 调用对应方法。所有未覆写的 JDBC hook 仍通过 [`ResultSetFilter`] 默认方法
/// 继续调用链。
///
/// 当前 `ExecContext` 尚未携带 Java `StatementProxy` 平台对象，所以监听器保留
/// SQL、入口种类、参数、事务和结果语义，但 Statement 实例身份仍是显式迁移
/// 缺口；本对象因此不能单独证明 Java 541 行实现已完整迁移。
#[derive(Debug, Clone)]
pub struct FilterEventAdapter<L = ()> {
    listener: L,
}

impl FilterEventAdapter<()> {
    /// 创建所有事件均为空操作的适配器。
    ///
    /// 对应 Java：匿名 `new FilterEventAdapter() {}`。
    #[must_use]
    pub const fn new() -> Self {
        Self { listener: () }
    }
}

impl<L> FilterEventAdapter<L> {
    /// 使用组合监听器创建适配器。
    ///
    /// 对应 Java：继承 `FilterEventAdapter` 并覆盖 protected 事件方法。
    /// 参数 `listener` 承载覆盖逻辑；返回绑定该监听器的适配器。
    #[must_use]
    pub const fn with_listener(listener: L) -> Self {
        Self { listener }
    }

    /// 返回组合监听器的共享引用。
    #[must_use]
    pub const fn listener(&self) -> &L {
        &self.listener
    }
}

impl Default for FilterEventAdapter<()> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<L> BeforeFilter for FilterEventAdapter<L>
where
    L: FilterEventListener + 'static,
{
    fn name(&self) -> &str {
        "FilterEventAdapter"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        match context.operation {
            ExecOperation::Execute => self.listener.statement_execute_before(context).await,
            ExecOperation::Query => self.listener.statement_execute_query_before(context).await,
            ExecOperation::Update => self.listener.statement_execute_update_before(context).await,
            ExecOperation::Batch => Ok(()),
        }
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        self.listener.statement_execute_batch_before(context).await
    }

    async fn connection_connect(
        &self,
        chain: &mut PhysicalConnectionConnectFilterChain<'_>,
        properties: &mut HashMap<String, String>,
    ) -> Result<PhysicalConnectionConnectResult, DruidError> {
        self.listener.connection_connect_before().await?;
        let result = chain.connection_connect(properties).await?;
        self.listener
            .connection_connect_after(result.connection_id())
            .await?;
        Ok(result)
    }

    async fn on_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        if event == &ConnectionEvent::Connect {
            self.listener.connection_connect_before().await
        } else {
            Ok(())
        }
    }

    async fn on_statement_event(&self, event: &StatementEvent) -> Result<(), DruidError> {
        match event {
            StatementEvent::CreateStatement => self.listener.statement_create_after().await,
            StatementEvent::PrepareStatement(sql) => {
                self.listener.statement_prepare_after(sql).await
            }
            StatementEvent::PrepareCall(sql) => {
                self.listener.statement_prepare_call_after(sql).await
            }
            StatementEvent::Execute(_)
            | StatementEvent::ExecuteQuery(_)
            | StatementEvent::ExecuteUpdate(_)
            | StatementEvent::Close
            | StatementEvent::ExecuteBatch => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl<L> AfterFilter for FilterEventAdapter<L>
where
    L: FilterEventListener + 'static,
{
    fn name(&self) -> &str {
        "FilterEventAdapter"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        let success = match result {
            Ok(execution) => match context.operation {
                ExecOperation::Execute => {
                    self.listener
                        .statement_execute_after(context, execution.row_count.is_some())
                        .await
                }
                ExecOperation::Query => self.listener.statement_execute_query_after(context).await,
                ExecOperation::Update => {
                    let update_count = match i32::try_from(execution.rows_affected) {
                        Ok(update_count) => update_count,
                        Err(_) => {
                            let error = DruidError::InvalidArgument(format!(
                                "update count exceeds JDBC int range: {}",
                                execution.rows_affected
                            ));
                            self.listener
                                .statement_execute_error_after(&context.sql, &error)
                                .await?;
                            return Err(error);
                        }
                    };
                    self.listener
                        .statement_execute_update_after(context, update_count)
                        .await
                }
                ExecOperation::Batch => Ok(()),
            },
            Err(error) => {
                return self
                    .listener
                    .statement_execute_error_after(&context.sql, error)
                    .await;
            }
        };

        if let Err(error) = success {
            self.listener
                .statement_execute_error_after(&context.sql, &error)
                .await?;
            return Err(error);
        }
        Ok(())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        let success = match result {
            Ok(update_counts) => {
                self.listener
                    .statement_execute_batch_after(context, update_counts)
                    .await
            }
            Err(error) => {
                return self
                    .listener
                    .statement_execute_error_after(context.sql, error)
                    .await;
            }
        };
        if let Err(error) = success {
            self.listener
                .statement_execute_error_after(context.sql, &error)
                .await?;
            return Err(error);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<L> ExtendedFilter for FilterEventAdapter<L>
where
    L: FilterEventListener + 'static,
{
    fn is_wrapper_for(&self, type_name: &str) -> bool {
        type_name == std::any::type_name::<Self>()
    }
}

impl<L> ResultSetFilter for FilterEventAdapter<L>
where
    L: FilterEventListener + 'static,
{
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.listener.result_set_open_after(context)
    }
}

impl<L> Wrapper for FilterEventAdapter<L>
where
    L: FilterEventListener + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}
