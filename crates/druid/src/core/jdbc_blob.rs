//! JDBC Blob 平台对象与物理 SPI。
//!
//! 对应 Java 平台对象：`java.sql.Blob`。Druid 只转发该资源对象，不拥有其
//! 字节存储；具体 SQLx、RBDC 或其他 Adapter 通过 `PhysicalBlob` 实现驱动语义。

use super::{DruidError, JdbcInputStream, JdbcOutputStream};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// 物理 Blob 的完整 JDBC 操作契约。
///
/// 参数保留 Java 的有符号 `long/int`，因此无效位置、长度和 offset 仍由真实
/// 驱动按 JDBC 规则报告，不在池化层静默归一化。
pub trait PhysicalBlob: fmt::Debug + Send + Sync {
    /// 返回具体驱动对象，供 Adapter 做安全向下转换。
    fn as_any(&self) -> &dyn Any;

    /// 返回 Blob 长度。对应 Java：`Blob#length()`。
    fn length(&self) -> Result<i64, DruidError>;

    /// 读取指定范围。对应 Java：`Blob#getBytes(long, int)`。
    fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError>;

    /// 打开完整二进制流。对应 Java：`Blob#getBinaryStream()`。
    fn get_binary_stream(&self) -> Result<JdbcInputStream, DruidError>;

    /// 定位字节模式。对应 Java：`Blob#position(byte[], long)`。
    fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError>;

    /// 定位另一 Blob。对应 Java：`Blob#position(Blob, long)`。
    fn position_blob(&self, pattern: &JdbcBlob, start: i64) -> Result<Option<i64>, DruidError>;

    /// 写入全部字节。对应 Java：`Blob#setBytes(long, byte[])`。
    fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError>;

    /// 写入字节子区间。对应 Java：`Blob#setBytes(long, byte[], int, int)`。
    fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError>;

    /// 打开定位写入流。对应 Java：`Blob#setBinaryStream(long)`。
    fn set_binary_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError>;

    /// 截断 Blob。对应 Java：`Blob#truncate(long)`。
    fn truncate(&self, length: i64) -> Result<(), DruidError>;

    /// 释放 Blob。对应 Java：`Blob#free()`。
    fn free(&self) -> Result<(), DruidError>;

    /// 返回 Blob 是否已经释放；供 Druid 生命周期与测试使用。
    fn is_freed(&self) -> bool;

    /// 打开范围二进制流。对应 Java：`Blob#getBinaryStream(long, long)`。
    fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcInputStream, DruidError>;
}

/// 对外 JDBC Blob 句柄。
///
/// Clone 保留 Java 对象引用语义；相等比较基于同一物理 Blob 身份，而不是对
/// 大对象内容做隐式读取。
#[derive(Clone)]
pub struct JdbcBlob {
    physical: Arc<dyn PhysicalBlob>,
}

impl JdbcBlob {
    /// 包装一个物理 Blob Adapter。
    pub fn new(physical: Arc<dyn PhysicalBlob>) -> Self {
        Self { physical }
    }

    /// 返回物理 Blob SPI。
    pub fn physical(&self) -> &dyn PhysicalBlob {
        self.physical.as_ref()
    }

    /// 返回 Blob 长度。对应 Java：`Blob#length()`。
    pub fn length(&self) -> Result<i64, DruidError> {
        self.physical.length()
    }

    /// 读取指定范围。对应 Java：`Blob#getBytes(long, int)`。
    pub fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError> {
        self.physical.get_bytes(position, length)
    }

    /// 打开完整二进制流。对应 Java：`Blob#getBinaryStream()`。
    pub fn get_binary_stream(&self) -> Result<JdbcInputStream, DruidError> {
        self.physical.get_binary_stream()
    }

    /// 定位字节模式。对应 Java：`Blob#position(byte[], long)`。
    pub fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_bytes(pattern, start)
    }

    /// 定位另一 Blob。对应 Java：`Blob#position(Blob, long)`。
    pub fn position_blob(&self, pattern: &JdbcBlob, start: i64) -> Result<Option<i64>, DruidError> {
        self.physical.position_blob(pattern, start)
    }

    /// 写入全部字节。对应 Java：`Blob#setBytes(long, byte[])`。
    pub fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError> {
        self.physical.set_bytes(position, bytes)
    }

    /// 写入字节子区间。对应 Java：`Blob#setBytes(long, byte[], int, int)`。
    pub fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        self.physical
            .set_bytes_range(position, bytes, offset, length)
    }

    /// 打开定位写入流。对应 Java：`Blob#setBinaryStream(long)`。
    pub fn set_binary_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError> {
        self.physical.set_binary_stream(position)
    }

    /// 截断 Blob。对应 Java：`Blob#truncate(long)`。
    pub fn truncate(&self, length: i64) -> Result<(), DruidError> {
        self.physical.truncate(length)
    }

    /// 释放 Blob。对应 Java：`Blob#free()`。
    pub fn free(&self) -> Result<(), DruidError> {
        self.physical.free()
    }

    /// 返回 Blob 是否已经释放。
    pub fn is_freed(&self) -> bool {
        self.physical.is_freed()
    }

    /// 打开范围二进制流。对应 Java：`Blob#getBinaryStream(long, long)`。
    pub fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcInputStream, DruidError> {
        self.physical.get_binary_stream_range(position, length)
    }
}

impl fmt::Debug for JdbcBlob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcBlob")
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for JdbcBlob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcBlob {}
