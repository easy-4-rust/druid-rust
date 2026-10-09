//! Druid-private ResultSet metadata proxy state.

use super::{ProxyAttributeValue, ProxyAttributes};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnType, ResultSetMetaData,
    ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
#[cfg(test)]
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Druid 内部的 ResultSet metadata 代理，保留 Java proxy attributes 语义。
///
/// 对应 Java: `com.alibaba.druid.proxy.jdbc.ResultSetMetaDataProxyImpl`。canonical
/// `druid-rdbc` metadata 保持纯粹；attributes 只存在于 Druid 逻辑代理边界。
#[allow(dead_code)]
pub(crate) struct ResultSetMetaDataProxy<T: ResultSetMetaData + ?Sized = dyn ResultSetMetaData> {
    raw: Arc<T>,
    attributes: Arc<ProxyAttributes>,
}

// No current public canonical method exposes the Java proxy attribute map. Keep this exact
// Druid-private contract available to internal filters and bind it with the unit test below.
#[allow(dead_code)]
impl<T: ResultSetMetaData + ?Sized> ResultSetMetaDataProxy<T> {
    /// 使用共享的 ResultSet metadata attribute store 创建代理。
    pub(crate) fn new(raw: Arc<T>, attributes: Arc<ProxyAttributes>) -> Self {
        Self { raw, attributes }
    }

    /// 返回该列是否明确允许 SQL NULL，保留一次原始 nullability 调用。
    /// 参数为 1-based 列下标；返回驱动结果或原始错误。对应 Java: ResultSetMetaData#isNullable。
    pub(crate) fn is_nullable(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        Ok(matches!(
            self.raw.nullability(column)?,
            ColumnNullability::Nullable
        ))
    }

    /// 返回 Java 三态可空性代码，未知状态为 2；不预读列描述符。
    /// 参数为列下标；返回 0/1/2 或驱动原始错误。对应 Java: ResultSetMetaData#isNullable。
    pub(crate) fn nullable_code(&self, column: ColumnIndex) -> Result<i32, RdbcError> {
        Ok(match self.raw.nullability(column)? {
            ColumnNullability::NoNulls => 0,
            ColumnNullability::Nullable => 1,
            _ => 2,
        })
    }

    /// 返回原列类型的 JDBC 数值，不猜测原标准未提供的整数宽度。
    /// 参数为列下标；返回固定 BASE 的类型代码或驱动错误。对应 Java: ResultSetMetaData#getColumnType。
    pub(crate) fn rdbc_type(&self, column: ColumnIndex) -> Result<i32, RdbcError> {
        Ok(match self.raw.column_type(column)? {
            ResultSetColumnType::Boolean => 16,
            ResultSetColumnType::Integer => -5,
            ResultSetColumnType::Float => 8,
            ResultSetColumnType::Decimal => 3,
            ResultSetColumnType::Date => 91,
            ResultSetColumnType::Time => 92,
            ResultSetColumnType::Timestamp => 93,
            ResultSetColumnType::Text => 12,
            ResultSetColumnType::Binary => -3,
            ResultSetColumnType::Jdbc(code) => code,
            _ => 1_111,
        })
    }

    /// 检查私有代理、未擦除后端或标准元数据类型身份。
    /// 泛型 U 为目标类型；返回是否匹配，不把任意类型泄露到公共标准视图。
    pub(crate) fn is_wrapper_for_type<U: ?Sized + 'static>(&self) -> bool {
        TypeId::of::<U>() == TypeId::of::<Self>()
            || TypeId::of::<U>() == TypeId::of::<T>()
            || TypeId::of::<U>() == TypeId::of::<dyn ResultSetMetaData>()
            || TypeId::of::<U>() == TypeId::of::<dyn Wrapper>()
            || self
                .raw
                .is_wrapper_for(Some(TypeId::of::<U>()))
                .unwrap_or(false)
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

// 具体对象引用仅用于固定 BASE 的指针身份断言；生产解包使用下方标准 Wrapper 视图。
#[cfg(test)]
impl<T: ResultSetMetaData> ResultSetMetaDataProxy<T> {
    /// 解包私有具体代理或未擦除的原后端，不创建副本。
    /// 泛型 U 为具体目标；返回原对象引用，不匹配返回参数错误。对应 Java: Wrapper#unwrap。
    pub(crate) fn unwrap_ref<U: Any>(&self) -> Result<&U, RdbcError> {
        let own: &dyn Any = self;
        let raw: &dyn Any = self.raw.as_ref();
        own.downcast_ref::<U>()
            .or_else(|| raw.downcast_ref::<U>())
            .ok_or_else(|| {
                RdbcError::InvalidArgument(
                    "metadata concrete type does not match target".to_owned(),
                )
            })
    }
}

impl<T: ResultSetMetaData + ?Sized> Clone for ResultSetMetaDataProxy<T> {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.raw), Arc::clone(&self.attributes))
    }
}

impl<T: ResultSetMetaData + ?Sized> fmt::Debug for ResultSetMetaDataProxy<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResultSetMetaDataProxy")
            .field("backend_type", &std::any::type_name::<T>())
            .field("backend", &self.raw)
            .finish_non_exhaustive()
    }
}

impl<T: ResultSetMetaData + ?Sized, U: ResultSetMetaData + ?Sized>
    PartialEq<ResultSetMetaDataProxy<U>> for ResultSetMetaDataProxy<T>
{
    fn eq(&self, other: &ResultSetMetaDataProxy<U>) -> bool {
        // 后端身份由共享对象地址决定；标准视图的 vtable 不代表不同后端。
        std::ptr::addr_eq(Arc::as_ptr(&self.raw), Arc::as_ptr(&other.raw))
    }
}

impl<T: ResultSetMetaData + ?Sized> Eq for ResultSetMetaDataProxy<T> {}

impl Default for ResultSetMetaDataProxy<dyn ResultSetMetaData> {
    fn default() -> Self {
        Self::new(
            Arc::new(ResultSetMetaDataSnapshot::default()),
            Arc::new(ProxyAttributes::default()),
        )
    }
}

impl<T: ResultSetMetaData> Wrapper for ResultSetMetaDataProxy<T> {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn ResultSetMetaData>()
            || self.raw.is_wrapper_for(Some(target)).unwrap_or(false))
        .then_some(StandardUnwrapped::ResultSetMetaData(self.raw.as_ref()))
    }

    fn canonical_unwrap_mut(&mut self, target: TypeId) -> Option<StandardUnwrappedMut<'_>> {
        if target == TypeId::of::<dyn Wrapper>() {
            return Some(StandardUnwrappedMut::Wrapper(self));
        }
        if target == TypeId::of::<dyn ResultSetMetaData>()
            || self.raw.is_wrapper_for(Some(target)).unwrap_or(false)
        {
            // 共享不可变 Arc 不得伪装成独占后端；仅唯一所有者可以提供原可变视图。
            return Arc::get_mut(&mut self.raw)
                .map(|raw| StandardUnwrappedMut::ResultSetMetaData(raw));
        }
        None
    }
}

impl Wrapper for ResultSetMetaDataProxy<dyn ResultSetMetaData> {
    fn wrapper_view(&self) -> &dyn Wrapper {
        self
    }

    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn ResultSetMetaData>()
            || self.raw.is_wrapper_for(Some(target)).unwrap_or(false))
        .then_some(StandardUnwrapped::ResultSetMetaData(self.raw.as_ref()))
    }

    fn canonical_unwrap_mut(&mut self, target: TypeId) -> Option<StandardUnwrappedMut<'_>> {
        if target == TypeId::of::<dyn Wrapper>() {
            return Some(StandardUnwrappedMut::Wrapper(self));
        }
        if target == TypeId::of::<dyn ResultSetMetaData>()
            || self.raw.is_wrapper_for(Some(target)).unwrap_or(false)
        {
            return Arc::get_mut(&mut self.raw).map(StandardUnwrappedMut::ResultSetMetaData);
        }
        None
    }
}

impl<T: ResultSetMetaData + ?Sized> ResultSetMetaData for ResultSetMetaDataProxy<T>
where
    ResultSetMetaDataProxy<T>: Wrapper,
{
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

#[cfg(test)]
mod metadata_identity_tests;
