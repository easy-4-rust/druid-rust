//! JDBC Callable 使用的 URL 平台值。
//!
//! 对应 Java 平台对象：`java.net.URL`。Druid 池化层只负责把已构造 URL
//! 原样传给驱动或返回给调用方，不执行 DNS、连接或协议处理。

/// 保留 Java URL external form 的值对象。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JdbcUrl {
    external_form: String,
}

impl JdbcUrl {
    /// 从 Java `URL#toExternalForm()` 等价值创建 URL。
    pub fn new(external_form: impl Into<String>) -> Self {
        Self {
            external_form: external_form.into(),
        }
    }

    /// 返回未改写的 external form。
    pub fn external_form(&self) -> &str {
        &self.external_form
    }
}

impl From<String> for JdbcUrl {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for JdbcUrl {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
