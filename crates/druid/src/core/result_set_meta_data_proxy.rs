//! Druid-private ResultSet metadata proxy state.

use super::{ProxyAttributeValue, ProxyAttributes};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnType, ResultSetMetaData,
};
use druid_rdbc::result_set::ColumnIndex;
use std::collections::HashMap;
use std::sync::Arc;

/// Druid 内部的 ResultSet metadata 代理，保留 Java proxy attributes 语义。
///
/// 对应 Java: `com.alibaba.druid.proxy.jdbc.ResultSetMetaDataProxyImpl`。canonical
/// `druid-rdbc` metadata 保持纯粹；attributes 只存在于 Druid 逻辑代理边界。
#[allow(dead_code)]
pub(crate) struct ResultSetMetaDataProxy {
    raw: Arc<dyn ResultSetMetaData>,
    attributes: Arc<ProxyAttributes>,
}

// No current public canonical method exposes the Java proxy attribute map. Keep this exact
// Druid-private contract available to internal filters and bind it with the unit test below.
#[allow(dead_code)]
impl ResultSetMetaDataProxy {
    /// 使用共享的 ResultSet metadata attribute store 创建代理。
    pub(crate) fn new(raw: Arc<dyn ResultSetMetaData>, attributes: Arc<ProxyAttributes>) -> Self {
        Self { raw, attributes }
    }

    /// 返回 attributes 数量。
    #[must_use]
    pub(crate) fn attributes_size(&self) -> usize {
        self.attributes.len()
    }

    /// 返回 attributes 快照。
    #[must_use]
    pub(crate) fn attributes(&self) -> HashMap<String, ProxyAttributeValue> {
        self.attributes.snapshot()
    }

    /// 返回指定 attribute。
    #[must_use]
    pub(crate) fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.attributes.get(key)
    }

    /// 保存或覆盖 attribute。
    pub(crate) fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.attributes.put(key, value)
    }

    /// 清空 attributes。
    pub(crate) fn clear_attributes(&self) {
        self.attributes.clear();
    }
}

druid_rdbc::impl_standard_wrapper!(ResultSetMetaDataProxy => ResultSetMetaData);

impl ResultSetMetaData for ResultSetMetaDataProxy {
    fn column_count(&self) -> usize {
        self.raw.column_count()
    }

    fn column(&self, column: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        self.raw.column(column)
    }

    fn is_auto_increment(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_auto_increment(column)
    }

    fn is_case_sensitive(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_case_sensitive(column)
    }

    fn is_searchable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_searchable(column)
    }

    fn is_currency(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_currency(column)
    }

    fn nullability(&self, column: ColumnIndex) -> Result<ColumnNullability, RdbcError> {
        self.raw.nullability(column)
    }

    fn is_signed(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_signed(column)
    }

    fn column_display_size(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.raw.column_display_size(column)
    }

    fn column_label(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.column_label(column)
    }

    fn column_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.column_name(column)
    }

    fn schema_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.schema_name(column)
    }

    fn precision(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.raw.precision(column)
    }

    fn scale(&self, column: ColumnIndex) -> Result<usize, RdbcError> {
        self.raw.scale(column)
    }

    fn table_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.table_name(column)
    }

    fn catalog_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.catalog_name(column)
    }

    fn column_type(&self, column: ColumnIndex) -> Result<ResultSetColumnType, RdbcError> {
        self.raw.column_type(column)
    }

    fn column_type_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.column_type_name(column)
    }

    fn is_read_only(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_read_only(column)
    }

    fn is_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_writable(column)
    }

    fn is_definitely_writable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        self.raw.is_definitely_writable(column)
    }

    fn column_class_name(&self, column: ColumnIndex) -> Result<String, RdbcError> {
        self.raw.column_class_name(column)
    }
}

#[cfg(test)]
mod tests {
    use super::ResultSetMetaDataProxy;
    use crate::core::{ProxyAttributeValue, ProxyAttributes};
    use druid_rdbc::metadata::ResultSetMetaDataSnapshot;
    use std::sync::Arc;

    #[test]
    fn result_set_meta_data_proxy_attributes() {
        let proxy = ResultSetMetaDataProxy::new(
            Arc::new(ResultSetMetaDataSnapshot::default()),
            Arc::new(ProxyAttributes::default()),
        );

        assert_eq!(proxy.attributes_size(), 0);
        assert!(proxy.attributes().is_empty());
        assert!(proxy.attribute("key").is_none());
    }

    #[test]
    fn result_set_meta_data_proxy_put_and_clear() {
        let attributes = Arc::new(ProxyAttributes::default());
        let proxy = ResultSetMetaDataProxy::new(
            Arc::new(ResultSetMetaDataSnapshot::default()),
            Arc::clone(&attributes),
        );

        assert_eq!(proxy.attributes_size(), 0);
        assert!(proxy.attributes().is_empty());
        assert!(proxy.attribute("key").is_none());
        proxy.put_attribute("key", ProxyAttributeValue::new("value".to_owned()));
        assert_eq!(proxy.attributes_size(), 1);
        assert!(proxy.attribute("key").is_some());

        proxy.clear_attributes();
        assert_eq!(proxy.attributes_size(), 0);
        assert!(proxy.attribute("key").is_none());
    }
}
