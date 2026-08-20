//! Java WrapperAdapter/PoolableWrapper 的差分语义契约。

extern crate druid_core as druid;
use druid::core::{PoolableWrapper, Unwrapped, Wrapper, WrapperAdapter, WrapperExt};
use std::any::{Any, TypeId};

#[derive(Debug)]
struct RawConnection;

impl Wrapper for RawConnection {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct UnsupportedType;

#[derive(Debug)]
struct SupportedInterface;

#[derive(Debug)]
struct AssignableWrapper;

impl Wrapper for AssignableWrapper {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_instance_of(&self, iface: TypeId) -> bool {
        iface == TypeId::of::<Self>() || iface == TypeId::of::<SupportedInterface>()
    }
}

#[derive(Debug)]
struct StatementConnection {
    connection: RawConnection,
}

impl Wrapper for StatementConnection {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_connection(&self) -> Option<&dyn Any> {
        Some(&self.connection)
    }
}

#[derive(Debug)]
struct DelegatedTarget;

#[derive(Debug)]
struct ProxyWrapper {
    delegated: DelegatedTarget,
}

impl Wrapper for ProxyWrapper {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        iface == Some(TypeId::of::<DelegatedTarget>())
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        (iface == Some(TypeId::of::<DelegatedTarget>()))
            .then_some(Unwrapped::Object(&self.delegated))
    }

    fn is_wrapper_proxy(&self) -> bool {
        true
    }
}

#[test]
fn wrapper_adapter_matches_java_null_self_and_foreign_type_semantics() {
    let adapter = WrapperAdapter::new();

    assert!(!adapter.is_wrapper_for(None));
    assert!(adapter.unwrap(None).is_none());
    assert!(adapter.is_wrapper_for_type::<WrapperAdapter>());
    assert!(adapter.unwrap_ref::<WrapperAdapter>().is_some());
    assert!(!adapter.is_wrapper_for_type::<UnsupportedType>());
    assert!(adapter.unwrap_ref::<UnsupportedType>().is_none());

    let object = adapter
        .unwrap(Some(TypeId::of::<WrapperAdapter>()))
        .expect("默认 Wrapper 必须解包自身");
    assert_eq!(format!("{object:?}"), "Unwrapped::Object");
    assert!(object.physical_connection().is_none());
}

#[test]
fn poolable_wrapper_matches_java_direct_and_null_wrapper_semantics() {
    let wrapper = PoolableWrapper::new(RawConnection);

    assert!(!wrapper.is_wrapper_for(None));
    assert!(wrapper.unwrap(None).is_none());
    assert!(wrapper.is_wrapper_for_type::<PoolableWrapper>());
    assert!(wrapper.unwrap_ref::<PoolableWrapper>().is_some());
    assert!(wrapper.is_wrapper_for_type::<RawConnection>());
    assert!(wrapper.unwrap_ref::<RawConnection>().is_some());
    assert!(!wrapper.is_wrapper_for_type::<UnsupportedType>());
    assert!(wrapper.unwrap_ref::<UnsupportedType>().is_none());
    assert!(wrapper.wrapped().is_some());
    assert!(format!("{wrapper:?}").contains("has_wrapper: true"));

    let empty = PoolableWrapper::from_optional(None);
    assert!(!empty.is_wrapper_for_type::<PoolableWrapper>());
    assert!(empty.unwrap_ref::<PoolableWrapper>().is_none());
    assert!(empty.wrapped().is_none());
    assert!(format!("{empty:?}").contains("has_wrapper: false"));

    let assignable = PoolableWrapper::new(AssignableWrapper);
    assert!(assignable.is_wrapper_for_type::<SupportedInterface>());
    let unwrapped = assignable
        .unwrap(Some(TypeId::of::<SupportedInterface>()))
        .expect("普通 Wrapper 必须按 isInstance 语义暴露底层对象");
    assert!(unwrapped.downcast_ref::<AssignableWrapper>().is_some());
}

#[test]
fn poolable_wrapper_preserves_statement_connection_and_proxy_delegation_order() {
    let statement = PoolableWrapper::new(StatementConnection {
        connection: RawConnection,
    });
    assert!(statement.is_wrapper_for_type::<RawConnection>());
    assert!(statement.unwrap_ref::<RawConnection>().is_some());
    assert!(!statement.is_wrapper_for_type::<UnsupportedType>());
    assert!(statement.unwrap_ref::<UnsupportedType>().is_none());

    let proxy = PoolableWrapper::new(ProxyWrapper {
        delegated: DelegatedTarget,
    });
    assert!(proxy.is_wrapper_for_type::<ProxyWrapper>());
    assert!(proxy.unwrap_ref::<ProxyWrapper>().is_some());
    assert!(proxy.is_wrapper_for_type::<DelegatedTarget>());
    assert!(proxy.unwrap_ref::<DelegatedTarget>().is_some());
    assert!(!proxy.is_wrapper_for_type::<UnsupportedType>());
}
