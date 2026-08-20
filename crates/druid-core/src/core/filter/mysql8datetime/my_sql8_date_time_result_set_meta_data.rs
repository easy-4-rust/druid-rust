//! 对应 Java：
//! `com.alibaba.druid.filter.mysql8datetime.MySQL8DateTimeResultSetMetaData`。

use crate::core::{DruidError, ResultSetMetaData};
use std::ops::Deref;

/// 将 `MySQL` 8 DATETIME 的 Java 类名恢复为 `java.sql.Timestamp`。
///
/// 其余方法经 `Deref` 原样委托给底层 metadata；底层物理 Wrapper 身份不会
/// 被 eager 复制或丢失，对应 Java 装饰器的逐方法委托语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MySQL8DateTimeResultSetMetaData {
    result_set_meta_data: ResultSetMetaData,
}

impl MySQL8DateTimeResultSetMetaData {
    /// 包装原始结果集 metadata。
    #[must_use]
    pub fn new(result_set_meta_data: ResultSetMetaData) -> Self {
        Self {
            result_set_meta_data,
        }
    }

    /// 返回兼容后的列类名。
    ///
    /// 仅将 `java.time.LocalDateTime` 改为 `java.sql.Timestamp`。
    pub fn column_class_name(&self, column: usize) -> Result<String, DruidError> {
        let class_name = self.result_set_meta_data.column_class_name(column)?;
        if class_name == "java.time.LocalDateTime" {
            Ok("java.sql.Timestamp".to_owned())
        } else {
            Ok(class_name)
        }
    }

    /// 转换为 Filter 链使用的 metadata 句柄。
    #[must_use]
    pub fn into_result_set_meta_data(self) -> ResultSetMetaData {
        self.result_set_meta_data
            .with_mysql8_datetime_compatibility()
    }
}

impl Deref for MySQL8DateTimeResultSetMetaData {
    type Target = ResultSetMetaData;

    fn deref(&self) -> &Self::Target {
        &self.result_set_meta_data
    }
}
