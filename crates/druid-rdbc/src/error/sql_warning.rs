//! RDBC `SQLWarning` 的平台值对象。
//!
//! 对应 Java：`java.sql.SQLWarning`。警告不是普通字符串：必须保留
//! SQLState、vendor code 和 warning 链。

/// 数据库操作产生的非致命警告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlWarning {
    message: String,
    sql_state: Option<String>,
    error_code: i32,
    next_warning: Option<Box<SqlWarning>>,
}

impl SqlWarning {
    /// 创建警告。对应 Java：`SQLWarning(String, String, int)`。
    pub fn new(message: impl Into<String>, sql_state: Option<String>, error_code: i32) -> Self {
        Self {
            message: message.into(),
            sql_state,
            error_code,
            next_warning: None,
        }
    }

    /// 返回警告消息。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 返回可空 `SQLState`。
    pub fn sql_state(&self) -> Option<&str> {
        self.sql_state.as_deref()
    }

    /// 返回驱动 vendor code。
    pub fn error_code(&self) -> i32 {
        self.error_code
    }

    /// 返回下一条警告。
    pub fn next_warning(&self) -> Option<&SqlWarning> {
        self.next_warning.as_deref()
    }

    /// 设置下一条警告。对应 Java：`SQLWarning#setNextWarning`。
    pub fn set_next_warning(&mut self, next_warning: SqlWarning) {
        self.next_warning = Some(Box::new(next_warning));
    }
}
