use super::standard_resource_value::StandardResourceValue;
use crate::error::RdbcError;
use crate::resource::{
    Clob, ClobPattern, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter,
    ResourceOperations,
};
use crate::types::RdbcString;
use std::{fmt, sync::Arc};
/// 字符大对象共享值句柄；对应 Java: java.sql.Clob。
/// 克隆保留同一Send标准对象、身份与释放确认，不提前物化字符。
#[derive(Clone)]
pub struct RdbcClob {
    value: Arc<StandardResourceValue<dyn Clob>>,
}
impl RdbcClob {
    /// 接管参数value，返回保留原驱动对象的Clob句柄。
    pub(crate) fn from_standard(value: Box<dyn Clob>) -> Self {
        Self::from_standard_named(value, "Clob")
    }
    /// 接管参数value并保存name标准类型名，返回共享值；NClob视图保留相同状态。
    pub(crate) fn from_standard_named(value: Box<dyn Clob>, name: &'static str) -> Self {
        let capabilities = value.capabilities() & ResourceOperations::clob();
        Self {
            value: Arc::new(StandardResourceValue::new(value, capabilities, name)),
        }
    }
    /// 返回UTF-16字符单元长度，无参数；失败返回原数据库错误。 对应 Java: Clob#length。
    pub async fn length(&self) -> Result<i64, RdbcError> {
        self.value
            .lock(ResourceOperations::READ, "Clob#length")
            .await?
            .length()
            .await
    }
    /// 读取1-based参数position起至多length个UTF-16单元，返回保留孤立代理项的原值。 对应 Java: Clob#getSubString。
    pub async fn get_sub_string(
        &self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::RANGE,
                "Clob#getSubString",
            )
            .await?
            .get_sub_string(position, length)
            .await
    }
    /// 返回原驱动字符读取流，无参数；不提前读取或转换UTF-16。 对应 Java: Clob#getCharacterStream。
    pub async fn get_character_stream(&self) -> Result<RdbcReader, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::STREAM,
                "Clob#getCharacterStream",
            )
            .await?
            .get_character_stream()
            .await
    }
    /// 返回原驱动ASCII读取流，无参数；编码策略由驱动决定。 对应 Java: Clob#getAsciiStream。
    pub async fn get_ascii_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::STREAM,
                "Clob#getAsciiStream",
            )
            .await?
            .get_ascii_stream()
            .await
    }
    /// 从1-based参数start搜索pattern，返回位置、None或原错误。 对应 Java: Clob#position(String,long)。
    pub async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::SEARCH,
                "Clob#position(String,long)",
            )
            .await?
            .position_string(pattern, start)
            .await
    }
    /// 将value写至1-based参数position，返回UTF-16单元数或原错误。 对应 Java: Clob#setString。
    pub async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, RdbcError> {
        self.value
            .lock(ResourceOperations::WRITE, "Clob#setString")
            .await?
            .set_string(position, value)
            .await
    }
    /// 将value的0-based offset起length单元写至1-based position，返回实际写入数。 对应 Java: Clob#setString(long,String,int,int)。
    pub async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::RANGE,
                "Clob#setString(long,String,int,int)",
            )
            .await?
            .set_string_range(position, value, offset, length)
            .await
    }
    /// 在1-based参数position返回原驱动ASCII写入流，不提前写入。 对应 Java: Clob#setAsciiStream。
    pub async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::STREAM,
                "Clob#setAsciiStream",
            )
            .await?
            .set_ascii_stream(position)
            .await
    }
    /// 在1-based参数position返回原驱动UTF-16字符写入器。 对应 Java: Clob#setCharacterStream。
    pub async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::STREAM,
                "Clob#setCharacterStream",
            )
            .await?
            .set_character_stream(position)
            .await
    }
    /// 截断至参数length个UTF-16单元，返回成功或原错误。 对应 Java: Clob#truncate。
    pub async fn truncate(&self, length: i64) -> Result<(), RdbcError> {
        self.value
            .lock(
                ResourceOperations::WRITE | ResourceOperations::TRUNCATE,
                "Clob#truncate",
            )
            .await?
            .truncate(length)
            .await
    }
    /// 返回1-based参数position起length单元的原字符流或原错误。 对应 Java: Clob#getCharacterStream(long,long)。
    pub async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        self.value
            .lock(
                ResourceOperations::READ | ResourceOperations::STREAM | ResourceOperations::RANGE,
                "Clob#getCharacterStream(long,long)",
            )
            .await?
            .get_character_stream_range(position, length)
            .await
    }
    /// 从1-based参数start搜索pattern所引用的标准Clob，返回位置、None或原错误。
    /// 对应 Java: Clob#position(Clob,long)。保留自身别名，不预读取模式内容。
    pub async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let required = ResourceOperations::READ | ResourceOperations::SEARCH;
        let operation = "Clob#position(Clob,long)";
        // 先检查目标，避免模式的锁序改变目标关闭/能力错误的优先级。
        self.value.require(required, operation)?;
        if Arc::ptr_eq(&self.value, &pattern.value) {
            // 单次独占借用即可表达自身模式，实际搜索与错误仍交给驱动。
            return self
                .value
                .lock(required, operation)
                .await?
                .position_clob(ClobPattern::SameResource, start)
                .await;
        }
        // 两个资源按稳定身份排序取锁，避免a搜索b和b搜索a反向等待。
        if true {
            let mut target = self.value.lock(required, operation).await?;
            let mut pattern = pattern
                .value
                .lock(ResourceOperations::empty(), operation)
                .await?;
            target
                .position_clob(ClobPattern::Other(pattern.as_mut()), start)
                .await
        } else {
            let mut pattern = pattern
                .value
                .lock(ResourceOperations::empty(), operation)
                .await?;
            let mut target = self.value.lock(required, operation).await?;
            target
                .position_clob(ClobPattern::Other(pattern.as_mut()), start)
                .await
        }
    }

    /// 幂等释放，无参数；失败可重试，取消后拒绝读取直到释放确认。对应 Java: Clob#free。
    pub async fn free(&self) -> Result<(), RdbcError> {
        self.value.free("Clob#free", |value| value.free()).await
    }
    /// 返回是否已确认成功释放，无参数（Rust扩展）。
    #[must_use]
    pub fn is_freed(&self) -> bool {
        self.value.is_freed()
    }
    /// 返回稳定共享身份，无参数（Rust扩展）。
    #[must_use]
    pub fn resource_id(&self) -> &str {
        self.value.resource_id()
    }
    /// 返回驱动声明与Clob标准操作的交集，无参数（Rust扩展）。
    #[must_use]
    pub fn capabilities(&self) -> ResourceOperations {
        self.value.capabilities()
    }
}
impl fmt::Debug for RdbcClob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdbcClob")
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}
impl PartialEq for RdbcClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}
impl Eq for RdbcClob {}
