use crate::error::RdbcError;
use crate::resource::{SqlData, SqlOutput};
use crate::types::OpaqueObjectValue;
use std::any::Any;
use std::fmt;

/// 自定义映射所得应用对象的共享身份；独占访问只要求应用对象Send。
/// 对应 Java: java.sql.SQLData（java.lang.Object返回值的Rust句柄扩展）。
pub struct SqlDataObject {
    class_name: String,
    value: tokio::sync::Mutex<Box<dyn SqlData>>,
}

impl SqlDataObject {
    /// 包装已成功readSQL的应用实例；参数为类名及唯一对象，返回共享句柄内容。
    pub(crate) fn new(class_name: String, value: Box<dyn SqlData>) -> Self {
        Self {
            class_name,
            value: tokio::sync::Mutex::new(value),
        }
    }

    /// 返回同一应用实例的限定SQL类型名，无参数；保留原错误。
    /// 对应 Java: SQLData#getSQLTypeName。
    pub async fn sql_type_name(&self) -> Result<String, RdbcError> {
        self.value.lock().await.sql_type_name().await
    }

    /// 将同一应用实例按声明顺序写入参数output；返回成功或原错误。
    /// 对应 Java: SQLData#writeSQL。
    pub async fn write_sql(&self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        self.value.lock().await.write_sql(output).await
    }
}

impl fmt::Debug for SqlDataObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlDataObject")
            .field("class_name", &self.class_name)
            .finish_non_exhaustive()
    }
}

impl OpaqueObjectValue for SqlDataObject {
    fn class_name(&self) -> &str {
        &self.class_name
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
