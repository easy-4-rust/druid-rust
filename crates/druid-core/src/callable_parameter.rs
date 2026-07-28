//! CallableStatement 参数定位。
//!
//! 对应 Java 平台依赖：`java.sql.CallableStatement` 的 `parameterIndex` /
//! `parameterName` 两组重载。

use crate::DruidError;

/// 存储过程参数的索引或名称。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableParameter {
    /// JDBC 从 1 开始的参数索引。
    Index(usize),
    /// 驱动识别的参数名称。
    Name(String),
}

impl CallableParameter {
    /// 创建索引参数。
    ///
    /// # 参数
    /// - `parameter_index`：对应 Java 参数 `parameterIndex`，必须大于零。
    pub fn by_index(parameter_index: usize) -> Result<Self, DruidError> {
        if parameter_index == 0 {
            Err(DruidError::InvalidArgument(
                "parameterIndex must be greater than zero".to_string(),
            ))
        } else {
            Ok(Self::Index(parameter_index))
        }
    }

    /// 创建命名参数。
    ///
    /// # 参数
    /// - `parameter_name`：对应 Java 参数 `parameterName`，不能为空。
    pub fn by_name(parameter_name: impl Into<String>) -> Result<Self, DruidError> {
        let parameter_name = parameter_name.into();
        if parameter_name.is_empty() {
            Err(DruidError::InvalidArgument(
                "parameterName is empty".to_string(),
            ))
        } else {
            Ok(Self::Name(parameter_name))
        }
    }
}
