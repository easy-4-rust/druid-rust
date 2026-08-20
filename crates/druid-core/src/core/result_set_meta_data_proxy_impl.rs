//! 对应 Java：`com.alibaba.druid.proxy.rdbc.ResultSetMetaDataProxyImpl`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/proxy/rdbc/ResultSetMetaDataProxyImpl.java`。

use super::{
    ProxyAttributeValue, ProxyAttributes, ResultSetMetaData, ResultSetMetaDataProxy, Unwrapped,
    Wrapper,
};
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// 携带 metadata/ResultSet 身份和 attributes 的结果集 metadata 代理。
pub struct ResultSetMetaDataProxyImpl {
    id: u64,
    result_set_id: u64,
    raw: ResultSetMetaData,
    attributes: ProxyAttributes,
}

impl ResultSetMetaDataProxyImpl {
    /// 创建 metadata 代理。
    pub fn new(raw: ResultSetMetaData, id: u64, result_set_id: u64) -> Self {
        Self {
            id,
            result_set_id,
            raw,
            attributes: ProxyAttributes::default(),
        }
    }

    /// 返回 attributes 数量。
    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.attributes.len()
    }

    /// 返回 attributes 快照。
    #[must_use]
    pub fn attributes(&self) -> HashMap<String, ProxyAttributeValue> {
        self.attributes.snapshot()
    }

    /// 返回指定 attribute。
    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.attributes.get(key)
    }

    /// 保存或覆盖 attribute。
    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.attributes.put(key, value)
    }

    /// 清空 attributes。
    pub fn clear_attributes(&self) {
        self.attributes.clear();
    }
}

impl ResultSetMetaDataProxy for ResultSetMetaDataProxyImpl {
    fn id(&self) -> u64 {
        self.id
    }

    fn result_set_meta_data_raw(&self) -> &ResultSetMetaData {
        &self.raw
    }

    fn result_set_id(&self) -> u64 {
        self.result_set_id
    }
}

impl Wrapper for ResultSetMetaDataProxyImpl {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_instance_of(&self, iface: TypeId) -> bool {
        iface == TypeId::of::<Self>() || self.raw.is_instance_of(iface)
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            Some(Unwrapped::Object(self))
        } else {
            self.raw.unwrap(Some(iface))
        }
    }
}
