//! 外部 RowSet 实现者的 canonical Wrapper 宏继承闭包。
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{RowSet, RowSetListener, SharedRowSet};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter};
use druid_rdbc::types::RdbcTypeMap;
use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
use std::any::TypeId;
use std::sync::{Arc, Mutex};
struct ExternalRowSet {
    inner: SharedRowSet,
}
druid_rdbc::impl_standard_wrapper!(ExternalRowSet => RowSet);
#[async_trait::async_trait]
impl ResultSet for ExternalRowSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.inner.next().await
    }
}
#[async_trait::async_trait]
impl RowSet for ExternalRowSet {
    fn set_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        RowSet::set_parameter(&mut self.inner, index, parameter)
    }
    fn set_named_parameter(
        &mut self,
        name: Option<&str>,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        RowSet::set_named_parameter(&mut self.inner, name, parameter)
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        RowSet::clear_parameters(&mut self.inner)
    }
    async fn execute(&mut self) -> Result<(), RdbcError> {
        RowSet::execute(&mut self.inner).await
    }
    fn add_row_set_listener(&mut self, listener: Arc<dyn RowSetListener>) {
        RowSet::add_row_set_listener(&mut self.inner, listener);
    }
    fn remove_row_set_listener(&mut self, listener: &Arc<dyn RowSetListener>) {
        RowSet::remove_row_set_listener(&mut self.inner, listener);
    }
    fn url(&self) -> Option<String> {
        RowSet::url(&self.inner)
    }
    fn set_url(&mut self, url: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_url(&mut self.inner, url)
    }
    fn data_source_name(&self) -> Option<String> {
        RowSet::data_source_name(&self.inner)
    }
    fn set_data_source_name(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_data_source_name(&mut self.inner, name)
    }
    fn username(&self) -> Option<String> {
        RowSet::username(&self.inner)
    }
    fn set_username(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_username(&mut self.inner, name)
    }
    fn password(&self) -> Option<String> {
        RowSet::password(&self.inner)
    }
    fn set_password(&mut self, password: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_password(&mut self.inner, password)
    }
    fn transaction_isolation(&self) -> TransactionIsolation {
        RowSet::transaction_isolation(&self.inner)
    }
    fn set_transaction_isolation(&mut self, level: i32) -> Result<(), RdbcError> {
        RowSet::set_transaction_isolation(&mut self.inner, level)
    }
    fn type_map(&self) -> Arc<Mutex<RdbcTypeMap>> {
        RowSet::type_map(&self.inner)
    }
    fn set_type_map(&mut self, map: Arc<Mutex<RdbcTypeMap>>) -> Result<(), RdbcError> {
        RowSet::set_type_map(&mut self.inner, map)
    }
    fn command(&self) -> Option<String> {
        RowSet::command(&self.inner)
    }
    fn set_command(&mut self, command: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_command(&mut self.inner, command)
    }
    fn is_read_only(&self) -> bool {
        RowSet::is_read_only(&self.inner)
    }
    fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        RowSet::set_read_only(&mut self.inner, value)
    }
    fn max_field_size(&self) -> i32 {
        RowSet::max_field_size(&self.inner)
    }
    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        RowSet::set_max_field_size(&mut self.inner, max)
    }
    fn max_rows(&self) -> i32 {
        RowSet::max_rows(&self.inner)
    }
    fn set_max_rows(&mut self, max: i32) -> Result<(), RdbcError> {
        RowSet::set_max_rows(&mut self.inner, max)
    }
    fn escape_processing(&self) -> bool {
        RowSet::escape_processing(&self.inner)
    }
    fn set_escape_processing(&mut self, enable: bool) -> Result<(), RdbcError> {
        RowSet::set_escape_processing(&mut self.inner, enable)
    }
    fn query_timeout(&self) -> i32 {
        RowSet::query_timeout(&self.inner)
    }
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        RowSet::set_query_timeout(&mut self.inner, seconds)
    }
    fn set_type(&mut self, type_code: i32) -> Result<(), RdbcError> {
        RowSet::set_type(&mut self.inner, type_code)
    }
    fn set_concurrency(&mut self, concurrency: i32) -> Result<(), RdbcError> {
        RowSet::set_concurrency(&mut self.inner, concurrency)
    }
}
#[test]
fn external_row_set_macro_exposes_same_leaf_parent_and_mutable_object() {
    let mut source = ExternalRowSet {
        inner: SharedRowSet::new(),
    };
    for target in [
        TypeId::of::<dyn RowSet>(),
        TypeId::of::<dyn ResultSet>(),
        TypeId::of::<ExternalRowSet>(),
        TypeId::of::<dyn Wrapper>(),
    ] {
        assert!(source.is_wrapper_for(Some(target)).unwrap());
    }
    let StandardUnwrapped::RowSet(leaf) = source.unwrap(Some(TypeId::of::<dyn RowSet>())).unwrap()
    else {
        panic!("RowSet leaf")
    };
    assert!(std::ptr::addr_eq(leaf, &source as &dyn RowSet));
    let StandardUnwrapped::ResultSet(parent) =
        source.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap()
    else {
        panic!("ResultSet parent")
    };
    assert!(std::ptr::addr_eq(parent, &source as &dyn ResultSet));
    let StandardUnwrappedMut::RowSet(leaf) = source
        .unwrap_mut(Some(TypeId::of::<ExternalRowSet>()))
        .unwrap()
    else {
        panic!("mutable leaf")
    };
    leaf.set_command(Some("external identity")).unwrap();
    assert_eq!(source.inner.command(), Some("external identity".into()));
    assert!(source.unwrap(None).is_err());
    assert!(source.unwrap_mut(None).is_err());
}

struct ExternalGenericRowSet<T: Send + 'static, const N: usize> {
    inner: SharedRowSet,
    marker: std::marker::PhantomData<T>,
}
druid_rdbc::impl_standard_wrapper!(generics [T, const N: usize] for ExternalGenericRowSet<T,N> where [T: Send + 'static] => RowSet);
#[async_trait::async_trait]
impl<T: Send + 'static, const N: usize> ResultSet for ExternalGenericRowSet<T, N> {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.inner.next().await
    }
}
#[async_trait::async_trait]
impl<T: Send + 'static, const N: usize> RowSet for ExternalGenericRowSet<T, N> {
    fn set_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        RowSet::set_parameter(&mut self.inner, index, parameter)
    }
    fn set_named_parameter(
        &mut self,
        name: Option<&str>,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        RowSet::set_named_parameter(&mut self.inner, name, parameter)
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        RowSet::clear_parameters(&mut self.inner)
    }
    async fn execute(&mut self) -> Result<(), RdbcError> {
        RowSet::execute(&mut self.inner).await
    }
    fn add_row_set_listener(&mut self, listener: Arc<dyn RowSetListener>) {
        RowSet::add_row_set_listener(&mut self.inner, listener);
    }
    fn remove_row_set_listener(&mut self, listener: &Arc<dyn RowSetListener>) {
        RowSet::remove_row_set_listener(&mut self.inner, listener);
    }
    fn url(&self) -> Option<String> {
        RowSet::url(&self.inner)
    }
    fn set_url(&mut self, url: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_url(&mut self.inner, url)
    }
    fn data_source_name(&self) -> Option<String> {
        RowSet::data_source_name(&self.inner)
    }
    fn set_data_source_name(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_data_source_name(&mut self.inner, name)
    }
    fn username(&self) -> Option<String> {
        RowSet::username(&self.inner)
    }
    fn set_username(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_username(&mut self.inner, name)
    }
    fn password(&self) -> Option<String> {
        RowSet::password(&self.inner)
    }
    fn set_password(&mut self, password: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_password(&mut self.inner, password)
    }
    fn transaction_isolation(&self) -> TransactionIsolation {
        RowSet::transaction_isolation(&self.inner)
    }
    fn set_transaction_isolation(&mut self, level: i32) -> Result<(), RdbcError> {
        RowSet::set_transaction_isolation(&mut self.inner, level)
    }
    fn type_map(&self) -> Arc<Mutex<RdbcTypeMap>> {
        RowSet::type_map(&self.inner)
    }
    fn set_type_map(&mut self, map: Arc<Mutex<RdbcTypeMap>>) -> Result<(), RdbcError> {
        RowSet::set_type_map(&mut self.inner, map)
    }
    fn command(&self) -> Option<String> {
        RowSet::command(&self.inner)
    }
    fn set_command(&mut self, command: Option<&str>) -> Result<(), RdbcError> {
        RowSet::set_command(&mut self.inner, command)
    }
    fn is_read_only(&self) -> bool {
        RowSet::is_read_only(&self.inner)
    }
    fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        RowSet::set_read_only(&mut self.inner, value)
    }
    fn max_field_size(&self) -> i32 {
        RowSet::max_field_size(&self.inner)
    }
    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        RowSet::set_max_field_size(&mut self.inner, max)
    }
    fn max_rows(&self) -> i32 {
        RowSet::max_rows(&self.inner)
    }
    fn set_max_rows(&mut self, max: i32) -> Result<(), RdbcError> {
        RowSet::set_max_rows(&mut self.inner, max)
    }
    fn escape_processing(&self) -> bool {
        RowSet::escape_processing(&self.inner)
    }
    fn set_escape_processing(&mut self, enable: bool) -> Result<(), RdbcError> {
        RowSet::set_escape_processing(&mut self.inner, enable)
    }
    fn query_timeout(&self) -> i32 {
        RowSet::query_timeout(&self.inner)
    }
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        RowSet::set_query_timeout(&mut self.inner, seconds)
    }
    fn set_type(&mut self, type_code: i32) -> Result<(), RdbcError> {
        RowSet::set_type(&mut self.inner, type_code)
    }
    fn set_concurrency(&mut self, concurrency: i32) -> Result<(), RdbcError> {
        RowSet::set_concurrency(&mut self.inner, concurrency)
    }
}
#[test]
fn external_generic_row_set_macro_retains_where_const_and_identity() {
    let mut source = ExternalGenericRowSet::<String, 7> {
        inner: SharedRowSet::new(),
        marker: std::marker::PhantomData,
    };
    for target in [
        TypeId::of::<dyn RowSet>(),
        TypeId::of::<dyn ResultSet>(),
        TypeId::of::<ExternalGenericRowSet<String, 7>>(),
        TypeId::of::<dyn Wrapper>(),
    ] {
        assert!(source.is_wrapper_for(Some(target)).unwrap());
    }
    let StandardUnwrapped::RowSet(leaf) = source.unwrap(Some(TypeId::of::<dyn RowSet>())).unwrap()
    else {
        panic!("generic leaf")
    };
    assert!(std::ptr::addr_eq(leaf, &source as &dyn RowSet));
    let StandardUnwrapped::ResultSet(parent) =
        source.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap()
    else {
        panic!("generic parent")
    };
    assert!(std::ptr::addr_eq(parent, &source as &dyn ResultSet));
    let StandardUnwrappedMut::RowSet(leaf) = source
        .unwrap_mut(Some(TypeId::of::<ExternalGenericRowSet<String, 7>>()))
        .unwrap()
    else {
        panic!("mutable generic leaf")
    };
    leaf.set_command(Some("generic identity")).unwrap();
    assert_eq!(source.inner.command(), Some("generic identity".into()));
    let StandardUnwrappedMut::ResultSet(parent) = source
        .unwrap_mut(Some(TypeId::of::<dyn ResultSet>()))
        .unwrap()
    else {
        panic!("mutable parent")
    };
    assert!(std::ptr::addr_eq(parent, &source as &dyn ResultSet));
    assert!(!source
        .is_wrapper_for(Some(TypeId::of::<
            dyn druid_rdbc::statement::PreparedStatement,
        >()))
        .unwrap());
}
