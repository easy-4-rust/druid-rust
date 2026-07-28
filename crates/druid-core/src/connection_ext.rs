//! 物理连接扩展能力。

use crate::error::DruidError;
use crate::meta_data::MetaData;
use crate::physical_connection::PhysicalConnection;
use std::collections::HashMap;
use std::time::Duration;

/// 物理连接扩展能力。
///
/// 对应 Java: `java.sql.Connection` 中 Statement、元数据和驱动扩展方法。
/// 不支持的能力必须返回明确错误，禁止伪造成功结果。
#[async_trait::async_trait]
pub trait ConnectionExt: PhysicalConnection {
    /// 创建普通 Statement 对象。
    async fn create_statement(&mut self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "create_statement",
        })
    }

    /// 创建 PreparedStatement 对象。
    async fn prepare_statement(
        &mut self,
        _sql: &str,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_statement",
        })
    }

    /// 创建 CallableStatement 对象。
    async fn prepare_call(
        &mut self,
        _sql: &str,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_call",
        })
    }

    /// 返回数据库与驱动元数据。
    fn get_meta_data(&self) -> Option<&MetaData> {
        None
    }

    /// 返回数据库产品名称。
    fn get_database_product_name(&self) -> Option<&str> {
        None
    }

    /// 返回数据库产品版本。
    fn get_database_product_version(&self) -> Option<&str> {
        None
    }

    /// 返回驱动主版本号。
    fn get_driver_major_version(&self) -> i32 {
        0
    }

    /// 返回驱动次版本号。
    fn get_driver_minor_version(&self) -> i32 {
        0
    }

    /// 返回结果集保持性。
    fn get_holdability(&self) -> i32 {
        1
    }

    /// 设置结果集保持性。
    async fn set_holdability(&mut self, _holdability: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_holdability",
        })
    }

    /// 设置客户端属性。
    async fn set_client_info(&mut self, _name: &str, _value: &str) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_client_info",
        })
    }

    /// 返回客户端属性。
    fn get_client_info(&self, _name: &str) -> Option<String> {
        None
    }

    /// 清除驱动警告。
    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "clear_warnings",
        })
    }

    /// 将 SQL 转换为驱动原生 SQL。
    async fn native_sql(&self, sql: &str) -> Result<String, DruidError> {
        Ok(sql.to_string())
    }

    /// 设置网络超时。
    async fn set_network_timeout(&mut self, _timeout: Duration) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_network_timeout",
        })
    }

    /// 返回网络超时毫秒数。
    fn get_network_timeout(&self) -> i32 {
        0
    }

    /// 返回驱动类型映射。
    fn get_type_map(&self) -> Option<HashMap<String, String>> {
        None
    }

    /// 设置驱动类型映射。
    async fn set_type_map(&mut self, _map: HashMap<String, String>) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "set_type_map",
        })
    }
}
