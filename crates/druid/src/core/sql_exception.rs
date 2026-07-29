//! JDBC `SQLException` 的 Rust 平台值对象。

/// SQL 异常 cause 的运行时类型。
///
/// Java sorter 会检查 cause 是否为 `SocketTimeoutException`，或按类名识别
/// `MySQL` `CommunicationsException`；Rust 驱动适配器须在边界处保留这两类信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlExceptionCause {
    /// 对应 Java `java.net.SocketTimeoutException`。
    SocketTimeout,
    /// 其他 cause 的 Java/驱动运行时类名。
    ClassName(String),
}

/// 不依赖具体驱动 crate 的 SQL 异常描述。
///
/// 对应 Java 平台对象：`java.sql.SQLException` 及其
/// `SQLRecoverableException` 子类型。该对象保留 vendor sorter 做出相同判断
/// 所需的全部可观察字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlException {
    error_code: i32,
    sql_state: Option<String>,
    message: Option<String>,
    class_name: String,
    assignable_types: Vec<String>,
    recoverable: bool,
    causes: Vec<SqlExceptionCause>,
}

impl SqlException {
    /// 创建普通 `SQLException` 描述。
    ///
    /// `sql_state` 与 `message` 均允许为 `None`，对应 Java getter 返回 null。
    pub fn new(error_code: i32, sql_state: Option<String>, message: Option<String>) -> Self {
        Self {
            error_code,
            sql_state,
            message,
            class_name: "java.sql.SQLException".to_string(),
            assignable_types: vec!["java.sql.SQLException".to_string()],
            recoverable: false,
            causes: Vec::new(),
        }
    }

    /// 创建只有 error code 与非空消息的驱动异常。
    pub fn driver(error_code: i32, message: impl Into<String>) -> Self {
        Self::new(error_code, None, Some(message.into()))
    }

    /// 设置 `SQLState`。
    #[must_use]
    pub fn with_sql_state(mut self, sql_state: impl Into<String>) -> Self {
        self.sql_state = Some(sql_state.into());
        self
    }

    /// 设置具体异常类名。
    #[must_use]
    pub fn with_class_name(mut self, class_name: impl Into<String>) -> Self {
        let class_name = class_name.into();
        self.class_name.clone_from(&class_name);
        if !self.assignable_types.contains(&class_name) {
            self.assignable_types.insert(0, class_name);
        }
        self
    }

    /// 追加一个可赋值的父类型。
    ///
    /// 对应 Java `instanceof` 所依赖的运行时继承关系。驱动 Adapter 在把具体
    /// 异常转换为本对象时，必须写入 sorter 会检查的父类或接口名。
    #[must_use]
    pub fn with_assignable_type(mut self, class_name: impl Into<String>) -> Self {
        let class_name = class_name.into();
        if !self.assignable_types.contains(&class_name) {
            self.assignable_types.push(class_name);
        }
        self
    }

    /// 标记为 Java `SQLRecoverableException`。
    #[must_use]
    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }

    /// 追加一层 cause；顺序必须从直接 cause 到根 cause。
    #[must_use]
    pub fn with_cause(mut self, cause: SqlExceptionCause) -> Self {
        self.causes.push(cause);
        self
    }

    /// 返回 vendor error code。
    pub fn error_code(&self) -> i32 {
        self.error_code
    }

    /// 返回可空 `SQLState`。
    pub fn sql_state(&self) -> Option<&str> {
        self.sql_state.as_deref()
    }

    /// 返回可空错误消息。
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// 返回异常运行时类名。
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// 判断异常是否可赋值给指定 Java 运行时类型。
    ///
    /// 对应 Java `exception instanceof TargetType`；与只比较具体类名不同，
    /// 该判断会遍历 Adapter 保存的父类型链。
    pub fn is_instance_of(&self, class_name: &str) -> bool {
        self.assignable_types
            .iter()
            .any(|assignable_type| assignable_type == class_name)
    }

    /// 返回具体类型到父类型的可赋值类型链。
    pub fn assignable_types(&self) -> &[String] {
        &self.assignable_types
    }

    /// 返回是否对应 `SQLRecoverableException`。
    pub fn is_recoverable(&self) -> bool {
        self.recoverable
    }

    /// 返回从直接 cause 开始的 cause 链。
    pub fn causes(&self) -> &[SqlExceptionCause] {
        &self.causes
    }
}
