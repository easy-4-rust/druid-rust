use super::standard_resource_value::StandardResourceValue;
use crate::error::RdbcError;
use crate::resource::{Blob, BlobPattern, RdbcInputStream, RdbcOutputStream, ResourceOperations};
use std::fmt;
use std::sync::Arc;

/// SQL二进制大对象的共享值句柄；对应 Java: java.sql.Blob。
/// 克隆共享唯一标准对象、引用身份与释放确认状态，只要求驱动Send。
#[derive(Clone)]
pub struct RdbcBlob {
    value: Arc<StandardResourceValue<dyn Blob>>,
}

impl RdbcBlob {
    /// 接管参数value所拥有的唯一标准Blob，返回尚未物化内容的共享句柄。
    pub(crate) fn from_standard(value: Box<dyn Blob>) -> Self {
        let capabilities = value.capabilities() & ResourceOperations::blob();
        Self {
            value: Arc::new(StandardResourceValue::new(value, capabilities, "Blob")),
        }
    }

    /// 返回字节长度，无参数；失败返回原数据库错误。对应 Java: Blob#length。
    pub async fn length(&self) -> Result<i64, RdbcError> {
        self.value
            .lock(ResourceOperations::READ, "Blob#length")
            .await?
            .length()
            .await
    }

    /// 从1-based参数position读取至多length字节，返回字节副本或原错误。
    /// 对应 Java: Blob#getBytes(long,int)。
    pub async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::RANGE,
                "Blob#getBytes",
            )
            .await?
            .get_bytes(position, length)
            .await
    }

    /// 返回原驱动完整读取流，无参数；不提前读取。对应 Java: Blob#getBinaryStream()。
    pub async fn get_binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::STREAM,
                "Blob#getBinaryStream",
            )
            .await?
            .get_binary_stream()
            .await
    }

    /// 从1-based参数start搜索pattern字节，返回匹配位置、None或原错误。
    /// 对应 Java: Blob#position(byte[],long)，Java的-1映射为None。
    pub async fn position_bytes(
        &self,
        pattern: &[u8],
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::SEARCH,
                "Blob#position(byte[],long)",
            )
            .await?
            .position_bytes(pattern, start)
            .await
    }

    /// 从1-based参数start搜索pattern所引用的标准Blob，返回位置、None或原错误。
    /// 对应 Java: Blob#position(Blob,long)。保留自身别名，不预读取模式内容。
    pub async fn position_blob(
        &self,
        pattern: &RdbcBlob,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let required = ResourceOperations::READ | ResourceOperations::SEARCH;
        let operation = "Blob#position(Blob,long)";
        // 先检查目标，避免模式的锁序改变目标关闭/能力错误的优先级。
        self.value.require(required, operation)?;
        if Arc::ptr_eq(&self.value, &pattern.value) {
            // 单次独占借用即可表达自身模式，实际搜索与错误仍交给驱动。
            return self
                .value
                .lock(required, operation)
                .await?
                .position_blob(BlobPattern::SameResource, start)
                .await;
        }
        // 两个资源按稳定身份排序取锁，避免a搜索b和b搜索a反向等待。
        if self.resource_id() < pattern.resource_id() {
            let mut target = self.value.lock(required, operation).await?;
            let mut pattern = pattern
                .value
                .lock(ResourceOperations::empty(), operation)
                .await?;
            target
                .position_blob(BlobPattern::Other(pattern.as_mut()), start)
                .await
        } else {
            let mut pattern = pattern
                .value
                .lock(ResourceOperations::empty(), operation)
                .await?;
            let mut target = self.value.lock(required, operation).await?;
            target
                .position_blob(BlobPattern::Other(pattern.as_mut()), start)
                .await
        }
    }

    /// 从1-based参数position覆盖写入bytes，返回写入字节数或原错误。
    /// 对应 Java: Blob#setBytes(long,byte[])。
    pub async fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, RdbcError> {
        self.value
            .lock(ResourceOperations::WRITE, "Blob#setBytes")
            .await?
            .set_bytes(position, bytes)
            .await
    }

    /// 将bytes的0-based offset起length字节写入1-based position，返回写入数或原错误。
    /// 对应 Java: Blob#setBytes(long,byte[],int,int)。
    pub async fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::RANGE,
                "Blob#setBytes(long,byte[],int,int)",
            )
            .await?
            .set_bytes_range(position, bytes, offset, length)
            .await
    }

    /// 在1-based参数position打开原驱动写入流，返回流或原错误，不提前写入。
    /// 对应 Java: Blob#setBinaryStream(long)。
    pub async fn set_binary_stream(&self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::STREAM,
                "Blob#setBinaryStream",
            )
            .await?
            .set_binary_stream(position)
            .await
    }

    /// 截断至参数length字节，返回成功或原错误。对应 Java: Blob#truncate(long)。
    pub async fn truncate(&self, length: i64) -> Result<(), RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::TRUNCATE,
                "Blob#truncate",
            )
            .await?
            .truncate(length)
            .await
    }

    /// 幂等释放资源，无参数；失败可重试，取消后拒绝读取直到重试确认。
    /// 对应 Java: Blob#free；池化资源的受监管释放仍由Druid承担。
    pub async fn free(&self) -> Result<(), RdbcError> {
        self.value.free("Blob#free", |value| value.free()).await
    }

    /// 返回是否已确认成功释放，无参数（Rust扩展）。
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.value.is_freed()
    }

    /// 返回1-based参数position起length字节的原驱动读取流或原错误。
    /// 对应 Java: Blob#getBinaryStream(long,long)。
    pub async fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::STREAM | ResourceOperations::RANGE,
                "Blob#getBinaryStream(long,long)",
            )
            .await?
            .get_binary_stream_range(position, length)
            .await
    }

    /// 返回稳定共享身份，无参数（Rust扩展）。
    #[must_use]
    pub fn resource_id(&self) -> &str {
        self.value.resource_id()
    }

    /// 返回驱动声明与Blob标准操作范围的交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations {
        self.value.capabilities()
    }
}

impl fmt::Debug for RdbcBlob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcBlob")
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}
impl PartialEq for RdbcBlob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}
impl Eq for RdbcBlob {}
