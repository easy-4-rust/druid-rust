//! 事务保存点。

/// 事务保存点句柄。
///
/// 对应 Java: `java.sql.Savepoint`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Savepoint {
    /// 保存点 ID。
    pub id: u64,
    /// 命名保存点的名称；匿名保存点为 `None`。
    pub name: Option<String>,
}

impl Savepoint {
    /// 返回匿名保存点 ID。命名保存点调用该方法返回 SQLException。
    pub fn get_savepoint_id(&self) -> Result<i32, crate::core::DruidError> {
        if self.name.is_some() {
            return Err(Self::access_error(
                "named savepoint does not have a numeric id",
            ));
        }
        i32::try_from(self.id).map_err(|_| Self::access_error("savepoint id exceeds RDBC int"))
    }

    /// 返回命名保存点名称。匿名保存点调用该方法返回 SQLException。
    pub fn get_savepoint_name(&self) -> Result<&str, crate::core::DruidError> {
        self.name
            .as_deref()
            .ok_or_else(|| Self::access_error("unnamed savepoint does not have a name"))
    }

    fn access_error(message: &str) -> crate::core::DruidError {
        crate::core::DruidError::SqlException(Box::new(crate::core::SqlException::new(
            0,
            Some("HY024".to_owned()),
            Some(message.to_owned()),
        )))
    }
}
