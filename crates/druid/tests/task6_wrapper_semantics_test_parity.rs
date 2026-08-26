//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical `Wrapper` / `WrapperExt` identity and leaf-inheritance semantics.

use druid::core::{
    CallableStatement, Connection, PreparedStatement, RdbcError, StandardUnwrapped, Statement,
    Wrapper, WrapperExt,
};
use std::any::TypeId;

#[derive(Debug)]
struct RawConnection;
druid_rdbc::impl_standard_wrapper!(RawConnection => Connection);

#[async_trait::async_trait]
impl Connection for RawConnection {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[derive(Debug)]
struct RawStatement;
druid_rdbc::impl_standard_wrapper!(RawStatement => Statement);

#[async_trait::async_trait]
impl Statement for RawStatement {}

#[derive(Debug)]
struct CallableLeaf;
druid_rdbc::impl_standard_wrapper!(CallableLeaf => CallableStatement);

#[async_trait::async_trait]
impl Statement for CallableLeaf {}

#[async_trait::async_trait]
impl PreparedStatement for CallableLeaf {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("CALL task6")
    }
}

#[async_trait::async_trait]
impl CallableStatement for CallableLeaf {}

#[derive(Debug)]
struct UnsupportedType;

#[test]
fn task6_wrapper_semantics_test_wrapper_adapter_matches_java_null_self_and_foreign_type_semantics()
{
    let adapter = RawConnection;

    assert!(adapter.is_wrapper_for(None).is_err());
    assert!(adapter.unwrap(None).is_err());
    assert!(adapter.is_wrapper_for_type::<RawConnection>());
    assert!(adapter.unwrap_ref::<RawConnection>().is_ok());
    assert!(!adapter.is_wrapper_for_type::<UnsupportedType>());
    assert!(adapter.unwrap_ref::<UnsupportedType>().is_err());

    assert!(adapter
        .is_wrapper_for(Some(TypeId::of::<dyn Connection>()))
        .unwrap());
    let object = adapter
        .unwrap(Some(TypeId::of::<dyn Connection>()))
        .expect("canonical Connection identity");
    assert!(matches!(object, StandardUnwrapped::Connection(_)));
    assert!(adapter
        .is_wrapper_for(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap());
    assert!(matches!(
        adapter.unwrap(Some(TypeId::of::<dyn Wrapper>())),
        Ok(StandardUnwrapped::Wrapper(_))
    ));
}

#[test]
fn task6_wrapper_semantics_test_poolable_wrapper_matches_java_direct_and_null_wrapper_semantics() {
    let wrapper = RawStatement;
    assert!(format!("{wrapper:?}").contains("RawStatement"));

    assert!(wrapper.is_wrapper_for(None).is_err());
    assert!(wrapper.unwrap(None).is_err());
    assert!(wrapper.is_wrapper_for_type::<RawStatement>());
    assert!(wrapper.unwrap_ref::<RawStatement>().is_ok());
    assert!(!wrapper.is_wrapper_for_type::<UnsupportedType>());
    assert!(wrapper.unwrap_ref::<UnsupportedType>().is_err());
    assert!(wrapper
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(matches!(
        wrapper.unwrap(Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(wrapper
        .is_wrapper_for(Some(TypeId::of::<dyn Wrapper>()))
        .unwrap());
    assert!(matches!(
        wrapper.unwrap(Some(TypeId::of::<dyn Wrapper>())),
        Ok(StandardUnwrapped::Wrapper(_))
    ));

    let empty: Option<RawStatement> = None;
    assert!(empty.is_none());
    assert!(empty.as_ref().is_none());
    assert_eq!(format!("{empty:?}"), "None");

    let foreign = RawConnection;
    assert!(foreign.is_wrapper_for_type::<RawConnection>());
    assert!(foreign
        .is_wrapper_for(Some(TypeId::of::<dyn Connection>()))
        .unwrap());
    assert!(matches!(
        foreign.unwrap(Some(TypeId::of::<dyn Connection>())),
        Ok(StandardUnwrapped::Connection(_))
    ));
    assert!(!foreign
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(foreign.unwrap(Some(TypeId::of::<dyn Statement>())).is_err());
}

#[test]
fn task6_wrapper_semantics_test_poolable_wrapper_preserves_statement_connection_and_proxy_delegation_order(
) {
    let statement = RawStatement;
    assert!(statement
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert!(matches!(
        statement.unwrap(Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(!statement
        .is_wrapper_for(Some(TypeId::of::<dyn Connection>()))
        .unwrap());
    assert!(statement
        .unwrap(Some(TypeId::of::<dyn Connection>()))
        .is_err());

    let proxy = CallableLeaf;
    assert!(proxy.is_wrapper_for_type::<CallableLeaf>());
    assert!(proxy.unwrap_ref::<CallableLeaf>().is_ok());
    assert!(matches!(
        proxy.unwrap(Some(TypeId::of::<dyn CallableStatement>())),
        Ok(StandardUnwrapped::CallableStatement(_))
    ));
    assert!(matches!(
        proxy.unwrap(Some(TypeId::of::<dyn PreparedStatement>())),
        Ok(StandardUnwrapped::PreparedStatement(_))
    ));
    assert!(matches!(
        proxy.unwrap(Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(proxy.unwrap(Some(TypeId::of::<dyn Connection>())).is_err());
}
