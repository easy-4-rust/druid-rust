//! 物理预编译语句 SPI。
//!
//! 对应 Java 平台依赖：`java.sql.PreparedStatement`。

use super::{
    DruidError, PhysicalCallableStatement, PhysicalStatementOptions, PreparedInputParameter,
    SqlTextStatement, SqlWarning, Value,
};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

/// 驱动预编译语句句柄。
///
/// 该对象只承载驱动 prepare 的结果；参数、Filter、缓存命中和逻辑关闭属于
/// `DruidPooledPreparedStatement`。SQLx/RBDC Adapter 可以保存各自的 statement
/// 元数据，但不得把驱动类型泄漏到公共 Druid API。
pub trait PhysicalPreparedStatement: Send + Sync {
    /// 返回原始 SQL。
    fn sql(&self) -> &str;

    /// 返回驱动 Adapter 用于类型检查的只读动态对象。
    fn as_any(&self) -> &dyn Any;

    /// 返回 CallableStatement 能力；普通 PreparedStatement 返回 `None`。
    fn as_callable(&self) -> Option<&dyn PhysicalCallableStatement> {
        None
    }

    /// 返回继承自 `Statement` 的 ResultSet 创建参数。
    fn statement_options(&self) -> PhysicalStatementOptions {
        PhysicalStatementOptions::default()
    }

    /// 返回最大字段字节数。
    fn max_field_size(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_max_field_size",
        })
    }

    /// 设置最大字段字节数。
    fn set_max_field_size(&self, _max: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_max_field_size",
        })
    }

    /// 返回最大结果行数。
    fn max_rows(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_max_rows",
        })
    }

    /// 设置最大结果行数。
    fn set_max_rows(&self, _max: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_max_rows",
        })
    }

    /// 设置 JDBC escape 处理开关。
    fn set_escape_processing(&self, _enabled: bool) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_escape_processing",
        })
    }

    /// 返回查询超时秒数。
    fn query_timeout(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_query_timeout",
        })
    }

    /// 设置查询超时秒数。
    fn set_query_timeout(&self, _seconds: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_query_timeout",
        })
    }

    /// 取消当前执行。
    fn cancel(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_cancel",
        })
    }

    /// 设置游标名称。
    fn set_cursor_name(&self, _name: &str) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_cursor_name",
        })
    }

    /// 设置抓取方向。
    fn set_fetch_direction(&self, _direction: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_fetch_direction",
        })
    }

    /// 返回抓取方向。
    fn fetch_direction(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_fetch_direction",
        })
    }

    /// 设置抓取行数。
    fn set_fetch_size(&self, _rows: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_set_fetch_size",
        })
    }

    /// 返回抓取行数。
    fn fetch_size(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_fetch_size",
        })
    }

    /// 设置执行完成后自动关闭。
    fn close_on_completion(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_close_on_completion",
        })
    }

    /// 返回执行完成后自动关闭状态。
    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_is_close_on_completion",
        })
    }

    /// 接收一个按 JDBC 下标设置的参数。
    ///
    /// 对应 Java：`PreparedStatement#setXxx(int, ...)`。默认实现只验证 Java
    /// 1-based 下标；需要在 setter 时验证类型、参数数量或保存驱动资源的 Adapter
    /// 应覆盖本方法。执行值仍由连接 Adapter 消费，池化层不会预读流或 LOB。
    fn set_parameter(
        &self,
        parameter_index: usize,
        _parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        if parameter_index == 0 {
            Err(DruidError::InvalidArgument(
                "parameterIndex must be at least 1".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// 清理上一次执行设置的参数。
    ///
    /// Rust API 每次执行显式传入参数；Adapter 有额外缓存时应覆盖本方法。
    fn clear_parameters(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 校验并接收一次批处理参数快照。
    ///
    /// 对应 Java：`PreparedStatement#addBatch()`。Druid wrapper 保存可移植的
    /// 参数快照；需要在 add 阶段分配驱动资源或执行参数校验的 Adapter 可覆盖。
    fn add_batch(&self, _params: &[Value]) -> Result<(), DruidError> {
        Ok(())
    }

    /// 校验并接收包含完整 JDBC setter 描述符的批处理参数快照。
    ///
    /// 默认实现仅接收快照；需要在 `addBatch` 阶段物化资源或进行驱动校验的
    /// Adapter 应覆盖本方法。
    fn add_parameter_batch(&self, _params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        Ok(())
    }

    /// 清理批处理参数。
    fn clear_batch(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 访问当前 ResultSet 前执行驱动 getter。
    ///
    /// 对应 Java：`PreparedStatement` 继承的 `Statement#getResultSet()`。
    fn get_result_set(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 访问当前更新计数前执行驱动 getter。
    fn get_update_count(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 访问 generated keys 前执行驱动 getter。
    fn get_generated_keys(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 推进到下一个 JDBC 结果。
    ///
    /// 对应 Java：PreparedStatement 继承的 `getMoreResults()` 与
    /// `getMoreResults(int)`。非法 current 必须在关闭旧 ResultSet 前失败。
    fn get_more_results(&self, current: Option<i32>) -> Result<(), DruidError> {
        if current.is_some_and(|value| !matches!(value, 1..=3)) {
            Err(DruidError::InvalidArgument(
                "current must be CLOSE_CURRENT_RESULT(1), KEEP_CURRENT_RESULT(2), or CLOSE_ALL_RESULTS(3)"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// 返回继承自 Statement 的警告链。
    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_warnings",
        })
    }

    /// 清除继承自 Statement 的警告链。
    fn clear_warnings(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_clear_warnings",
        })
    }

    /// 关闭物理语句句柄。
    fn close(&self) -> Result<(), DruidError>;

    /// 返回语句是否已经关闭。
    fn is_closed(&self) -> bool;
}

/// 仅保存 SQL 文本的驱动语句句柄。
///
/// 用于 RBDC 等由连接执行入口内部完成 prepare/cache 的生态接口；它不是公开
/// pooled statement，也不伪造执行结果。
pub struct SqlTextPreparedStatement {
    sql: String,
    closed: AtomicBool,
    statement: SqlTextStatement,
}

impl SqlTextPreparedStatement {
    /// 创建 SQL 文本句柄。
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
        }
    }
}

impl PhysicalPreparedStatement for SqlTextPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_options(&self) -> PhysicalStatementOptions {
        super::PhysicalStatement::options(&self.statement)
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        super::PhysicalStatement::max_field_size(&self.statement)
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        super::PhysicalStatement::set_max_field_size(&self.statement, max)
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        super::PhysicalStatement::max_rows(&self.statement)
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        super::PhysicalStatement::set_max_rows(&self.statement, max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), DruidError> {
        super::PhysicalStatement::set_escape_processing(&self.statement, enabled)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        super::PhysicalStatement::query_timeout(&self.statement)
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        super::PhysicalStatement::set_query_timeout(&self.statement, seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        super::PhysicalStatement::cancel(&self.statement)
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), DruidError> {
        super::PhysicalStatement::set_cursor_name(&self.statement, name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        super::PhysicalStatement::set_fetch_direction(&self.statement, direction)
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        super::PhysicalStatement::fetch_direction(&self.statement)
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        super::PhysicalStatement::set_fetch_size(&self.statement, rows)
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        super::PhysicalStatement::fetch_size(&self.statement)
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        super::PhysicalStatement::warnings(&self.statement)
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        super::PhysicalStatement::clear_warnings(&self.statement)
    }

    fn close_on_completion(&self) -> Result<(), DruidError> {
        super::PhysicalStatement::close_on_completion(&self.statement)
    }

    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        super::PhysicalStatement::is_close_on_completion(&self.statement)
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        super::PhysicalStatement::close(&self.statement)?;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
