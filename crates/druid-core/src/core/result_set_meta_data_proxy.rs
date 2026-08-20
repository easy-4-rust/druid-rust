//! 对应 Java：`com.alibaba.druid.proxy.rdbc.ResultSetMetaDataProxy`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/proxy/rdbc/ResultSetMetaDataProxy.java`。

use super::{ResultSetMetaData, Wrapper};

/// ResultSet metadata 的代理身份合同。
pub trait ResultSetMetaDataProxy: Wrapper {
    /// 返回数据源级 metadata ID。
    fn id(&self) -> u64;

    /// 返回底层 RDBC metadata 平台对象。
    fn result_set_meta_data_raw(&self) -> &ResultSetMetaData;

    /// 返回所属 ResultSet 的数据源级 ID。
    ///
    /// Java 返回强引用 `ResultSetProxy`；Rust 用稳定 ID 避免 metadata 与
    /// ResultSet 形成自引用所有权环。
    fn result_set_id(&self) -> u64;
}
