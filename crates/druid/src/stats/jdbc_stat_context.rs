//! 对应 Java 类：`com.alibaba.druid.stat.JdbcStatContext`。

/// 当前执行上下文携带的 JDBC 统计元数据。
///
/// 对应 Java: `com.alibaba.druid.stat.JdbcStatContext`。Java nullable 字段映射为
/// `Option<String>`，对象本身不依赖日志框架。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JdbcStatContext {
    name: Option<String>,
    file: Option<String>,
    sql: Option<String>,
    request_id: Option<String>,
    trace_enable: bool,
}

impl JdbcStatContext {
    /// 创建空统计上下文。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回 traceEnable。
    #[must_use]
    pub const fn is_trace_enable(&self) -> bool {
        self.trace_enable
    }

    /// 设置 traceEnable。
    pub fn set_trace_enable(&mut self, value: bool) {
        self.trace_enable = value;
    }

    /// 返回 requestId。
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// 设置 requestId。
    pub fn set_request_id(&mut self, value: Option<String>) {
        self.request_id = value;
    }

    /// 返回调用名称。
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// 设置调用名称。
    pub fn set_name(&mut self, value: Option<String>) {
        self.name = value;
    }

    /// 返回来源文件。
    #[must_use]
    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    /// 设置来源文件。
    pub fn set_file(&mut self, value: Option<String>) {
        self.file = value;
    }

    /// 返回 SQL。
    #[must_use]
    pub fn sql(&self) -> Option<&str> {
        self.sql.as_deref()
    }

    /// 设置 SQL。
    pub fn set_sql(&mut self, value: Option<String>) {
        self.sql = value;
    }
}
