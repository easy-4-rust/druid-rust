use crate::error::RdbcError;
use crate::wrapper::{StandardUnwrapped, StandardUnwrappedMut};
use std::any::{Any, TypeId};

/// JDBC `Wrapper` 的 object-safe canonical identity 标准。
///
/// 外部实现者应优先调用 [`crate::impl_standard_wrapper!`]；调用方只使用
/// [`Wrapper::is_wrapper_for`]、[`Wrapper::unwrap`] 与 [`Wrapper::unwrap_mut`] 的结构化 API，
/// 不直接调用隐藏钩子。metadata 具体类型目标仍只返回同一标准 metadata 视图。
pub trait Wrapper: Send {
    /// 实现者钩子：返回 canonical Wrapper identity。
    #[doc(hidden)]
    fn wrapper_view(&self) -> &dyn Wrapper;
    /// 实现者钩子：按目标返回 canonical interface view。
    #[doc(hidden)]
    fn canonical_unwrap(&self, target: TypeId) -> Option<StandardUnwrapped<'_>>;

    /// 实现者钩子：按目标返回原对象的独占可变标准视图；旧实现默认不提供。
    #[doc(hidden)]
    fn canonical_unwrap_mut(&mut self, _target: TypeId) -> Option<StandardUnwrappedMut<'_>> {
        None
    }

    /// 独占解包原标准对象。对应 Java：java.sql.Wrapper#unwrap。
    /// 参数 target 为目标标准接口；返回同一对象的可变视图，空目标为参数错误，不支持为0A000。
    fn unwrap_mut(
        &mut self,
        target: Option<TypeId>,
    ) -> Result<StandardUnwrappedMut<'_>, RdbcError> {
        let target = require_wrapper_target(target)?;
        self.canonical_unwrap_mut(target)
            .ok_or_else(|| RdbcError::feature_not_supported("wrapper_unwrap_mut_target"))
    }

    /// 判断对象是否实现目标 canonical interface。
    fn is_wrapper_for(&self, target: Option<TypeId>) -> Result<bool, RdbcError> {
        let target = require_wrapper_target(target)?;
        if target == TypeId::of::<dyn Wrapper>() {
            return Ok(true);
        }
        Ok(self.canonical_unwrap(target).is_some())
    }

    /// 解包为目标 canonical interface。
    fn unwrap(&self, target: Option<TypeId>) -> Result<StandardUnwrapped<'_>, RdbcError> {
        let target = require_wrapper_target(target)?;
        if target == TypeId::of::<dyn Wrapper>() {
            return Ok(StandardUnwrapped::Wrapper(self.wrapper_view()));
        }
        self.canonical_unwrap(target)
            .ok_or_else(|| RdbcError::feature_not_supported("wrapper_unwrap_target"))
    }
}

fn require_wrapper_target(target: Option<TypeId>) -> Result<TypeId, RdbcError> {
    target.ok_or_else(|| RdbcError::InvalidArgument("wrapper target must not be null".to_string()))
}

/// [`Wrapper`] 的 concrete Rust type 扩展；canonical core 不暴露 [`Any`]。
pub trait WrapperExt: Wrapper {
    /// 判断 concrete `Self` 是否为 `T`。
    fn is_wrapper_for_type<T: Any>(&self) -> bool
    where
        Self: Any + Sized,
    {
        TypeId::of::<Self>() == TypeId::of::<T>()
    }

    /// 将 concrete `Self` downcast 为 `T`。
    fn unwrap_ref<T: Any>(&self) -> Result<&T, RdbcError>
    where
        Self: Any + Sized,
    {
        let concrete: &dyn Any = self;
        concrete.downcast_ref::<T>().ok_or_else(|| {
            RdbcError::InvalidArgument("wrapper concrete type does not match target".to_string())
        })
    }
}

impl<T> WrapperExt for T where T: Wrapper + ?Sized {}

/// 为一个外部实现类型生成唯一的 canonical [`Wrapper`] 实现。
///
/// 每个具体类型只调用一次，并传入最具体的 canonical leaf；例如
/// `CallableStatement` 会自动展开 `PreparedStatement -> Statement` inheritance closure。
/// `DataSource`、`ConnectionPoolDataSource`、`XaDataSource` 与 `XaConnection` 的继承接口也会
/// 自动展开。带多个类型参数、const generic 与 where predicate 时使用
/// `generics [T, U, const N: usize] for Type<T, U, N> where [T: Bound] => ResultSet`。
///
/// 如果一个具体类型确实同时承担互不相关的多个 canonical roles，不要重复调用 macro；
/// 应手写唯一的 [`Wrapper`] implementation，并在 `canonical_unwrap` 中显式返回全部 roles。
#[macro_export]
macro_rules! impl_standard_wrapper {
    (generics [$($generics:tt)*] for $target:ty where [$($where_clause:tt)*] => $kind:ident) => {
        $crate::impl_standard_wrapper!(@emit [<$($generics)*>] $target where [$($where_clause)*] => $kind);
    };
    (impl<$generic:ident : $bound:path> $target:ty => $kind:ident) => {
        $crate::impl_standard_wrapper!(@emit [<$generic: $bound>] $target => $kind);
    };
    ($target:ty => $kind:ident) => {
        $crate::impl_standard_wrapper!(@emit [] $target => $kind);
    };
    (@emit [$($impl_generics:tt)*] $target:ty => Connection) => {
        impl $($impl_generics)* $crate::wrapper::Wrapper for $target {
            fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self }
            fn canonical_unwrap(&self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrapped<'_>> {
                (target == std::any::TypeId::of::<dyn $crate::connection::Connection>())
                    .then_some($crate::wrapper::StandardUnwrapped::Connection(self))
            } fn canonical_unwrap_mut(&mut self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if target == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }
                (target == std::any::TypeId::of::<dyn $crate::connection::Connection>())
                    .then_some($crate::wrapper::StandardUnwrappedMut::Connection(self))
             }
        }
    };
    (@emit [$($impl_generics:tt)*] $target:ty where [$($where_clause:tt)*] => Connection) => {
        impl $($impl_generics)* $crate::wrapper::Wrapper for $target where $($where_clause)* {
            fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self }
            fn canonical_unwrap(&self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrapped<'_>> {
                (target == std::any::TypeId::of::<dyn $crate::connection::Connection>()).then_some($crate::wrapper::StandardUnwrapped::Connection(self))
            } fn canonical_unwrap_mut(&mut self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if target == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }
                (target == std::any::TypeId::of::<dyn $crate::connection::Connection>()).then_some($crate::wrapper::StandardUnwrappedMut::Connection(self))
             }
        }
    };
    (@emit [$($g:tt)*] $t:ty where [$($w:tt)*] => ResultSet) => {
        impl $($g)* $crate::wrapper::Wrapper for $t where $($w)* {
            fn wrapper_view(&self)->&dyn $crate::wrapper::Wrapper { self }
            fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {
                (x==std::any::TypeId::of::<dyn $crate::result_set::ResultSet>()).then_some($crate::wrapper::StandardUnwrapped::ResultSet(self))
            } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }
                (x==std::any::TypeId::of::<dyn $crate::result_set::ResultSet>()).then_some($crate::wrapper::StandardUnwrappedMut::ResultSet(self))
             }
        }
    };
    (@emit [$($impl_generics:tt)*] $target:ty => Statement) => {
        impl $($impl_generics)* $crate::wrapper::Wrapper for $target {
            fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self }
            fn canonical_unwrap(&self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrapped<'_>> {
                (target == std::any::TypeId::of::<dyn $crate::statement::Statement>())
                    .then_some($crate::wrapper::StandardUnwrapped::Statement(self))
            } fn canonical_unwrap_mut(&mut self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if target == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }
                (target == std::any::TypeId::of::<dyn $crate::statement::Statement>())
                    .then_some($crate::wrapper::StandardUnwrappedMut::Statement(self))
             }
        }
    };
    (@emit [$($impl_generics:tt)*] $target:ty => PreparedStatement) => {
        impl $($impl_generics)* $crate::wrapper::Wrapper for $target {
            fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self }
            fn canonical_unwrap(&self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrapped<'_>> {
                if target == std::any::TypeId::of::<dyn $crate::statement::PreparedStatement>() { Some($crate::wrapper::StandardUnwrapped::PreparedStatement(self)) }
                else if target == std::any::TypeId::of::<dyn $crate::statement::Statement>() { Some($crate::wrapper::StandardUnwrapped::Statement(self)) }
                else { None }
            } fn canonical_unwrap_mut(&mut self, target: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if target == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }
                if target == std::any::TypeId::of::<dyn $crate::statement::PreparedStatement>() { Some($crate::wrapper::StandardUnwrappedMut::PreparedStatement(self)) }
                else if target == std::any::TypeId::of::<dyn $crate::statement::Statement>() { Some($crate::wrapper::StandardUnwrappedMut::Statement(self)) }
                else { None }
             }
        }
    };
    (@emit [$($g:tt)*] $t:ty => ResultSet) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self, x: std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { (x==std::any::TypeId::of::<dyn $crate::result_set::ResultSet>()).then_some($crate::wrapper::StandardUnwrapped::ResultSet(self)) } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  (x==std::any::TypeId::of::<dyn $crate::result_set::ResultSet>()).then_some($crate::wrapper::StandardUnwrappedMut::ResultSet(self))  }  } };
    (@emit [$($g:tt)*] $t:ty => ResultSetMetaData) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self, x: std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { (x==std::any::TypeId::of::<dyn $crate::metadata::ResultSetMetaData>() || x==std::any::TypeId::of::<Self>()).then_some($crate::wrapper::StandardUnwrapped::ResultSetMetaData(self)) } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  (x==std::any::TypeId::of::<dyn $crate::metadata::ResultSetMetaData>() || x==std::any::TypeId::of::<Self>()).then_some($crate::wrapper::StandardUnwrappedMut::ResultSetMetaData(self))  }  } };
    (@emit [$($g:tt)*] $t:ty => ParameterMetaData) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self, x: std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { (x==std::any::TypeId::of::<dyn $crate::metadata::ParameterMetaData>()).then_some($crate::wrapper::StandardUnwrapped::ParameterMetaData(self)) } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  (x==std::any::TypeId::of::<dyn $crate::metadata::ParameterMetaData>()).then_some($crate::wrapper::StandardUnwrappedMut::ParameterMetaData(self))  }  } };
    (@emit [$($g:tt)*] $t:ty => DatabaseMetaData) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self, x: std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { (x==std::any::TypeId::of::<dyn $crate::metadata::DatabaseMetaData>()).then_some($crate::wrapper::StandardUnwrapped::DatabaseMetaData(self)) } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  (x==std::any::TypeId::of::<dyn $crate::metadata::DatabaseMetaData>()).then_some($crate::wrapper::StandardUnwrappedMut::DatabaseMetaData(self))  }  } };
    (@emit [$($g:tt)*] $t:ty => DataSource) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self, x: std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { if x==std::any::TypeId::of::<dyn $crate::data_source::DataSource>() {Some($crate::wrapper::StandardUnwrapped::DataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrapped::CommonDataSource(self))} else {None} } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  if x==std::any::TypeId::of::<dyn $crate::data_source::DataSource>() {Some($crate::wrapper::StandardUnwrappedMut::DataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrappedMut::CommonDataSource(self))} else {None}  }  } };
    (@emit [$($g:tt)*] $t:ty => CallableStatement) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> { if x==std::any::TypeId::of::<dyn $crate::statement::CallableStatement>() {Some($crate::wrapper::StandardUnwrapped::CallableStatement(self))} else if x==std::any::TypeId::of::<dyn $crate::statement::PreparedStatement>() {Some($crate::wrapper::StandardUnwrapped::PreparedStatement(self))} else if x==std::any::TypeId::of::<dyn $crate::statement::Statement>() {Some($crate::wrapper::StandardUnwrapped::Statement(self))} else {None} } fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); }  if x==std::any::TypeId::of::<dyn $crate::statement::CallableStatement>() {Some($crate::wrapper::StandardUnwrappedMut::CallableStatement(self))} else if x==std::any::TypeId::of::<dyn $crate::statement::PreparedStatement>() {Some($crate::wrapper::StandardUnwrappedMut::PreparedStatement(self))} else if x==std::any::TypeId::of::<dyn $crate::statement::Statement>() {Some($crate::wrapper::StandardUnwrappedMut::Statement(self))} else {None}  }  } };
    (@emit [$($g:tt)*] $t:ty => CommonDataSource) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {(x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>()).then_some($crate::wrapper::StandardUnwrapped::CommonDataSource(self))} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } (x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>()).then_some($crate::wrapper::StandardUnwrappedMut::CommonDataSource(self)) }  } };
    (@emit [$($g:tt)*] $t:ty => PooledConnection) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {(x==std::any::TypeId::of::<dyn $crate::data_source::PooledConnection>()).then_some($crate::wrapper::StandardUnwrapped::PooledConnection(self))} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } (x==std::any::TypeId::of::<dyn $crate::data_source::PooledConnection>()).then_some($crate::wrapper::StandardUnwrappedMut::PooledConnection(self)) }  } };
    (@emit [$($g:tt)*] $t:ty => ConnectionPoolDataSource) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {if x==std::any::TypeId::of::<dyn $crate::data_source::ConnectionPoolDataSource>() {Some($crate::wrapper::StandardUnwrapped::ConnectionPoolDataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrapped::CommonDataSource(self))} else {None}} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } if x==std::any::TypeId::of::<dyn $crate::data_source::ConnectionPoolDataSource>() {Some($crate::wrapper::StandardUnwrappedMut::ConnectionPoolDataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrappedMut::CommonDataSource(self))} else {None} }  } };
    (@emit [$($g:tt)*] $t:ty => XaConnection) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {if x==std::any::TypeId::of::<dyn $crate::data_source::XaConnection>() {Some($crate::wrapper::StandardUnwrapped::XaConnection(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::PooledConnection>() {Some($crate::wrapper::StandardUnwrapped::PooledConnection(self))} else {None}} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } if x==std::any::TypeId::of::<dyn $crate::data_source::XaConnection>() {Some($crate::wrapper::StandardUnwrappedMut::XaConnection(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::PooledConnection>() {Some($crate::wrapper::StandardUnwrappedMut::PooledConnection(self))} else {None} }  } };
    (@emit [$($g:tt)*] $t:ty => XaDataSource) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {if x==std::any::TypeId::of::<dyn $crate::data_source::XaDataSource>() {Some($crate::wrapper::StandardUnwrapped::XaDataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrapped::CommonDataSource(self))} else {None}} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } if x==std::any::TypeId::of::<dyn $crate::data_source::XaDataSource>() {Some($crate::wrapper::StandardUnwrappedMut::XaDataSource(self))} else if x==std::any::TypeId::of::<dyn $crate::data_source::CommonDataSource>() {Some($crate::wrapper::StandardUnwrappedMut::CommonDataSource(self))} else {None} }  } };
    (@emit [$($g:tt)*] $t:ty => Driver) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {(x==std::any::TypeId::of::<dyn $crate::driver::Driver>()).then_some($crate::wrapper::StandardUnwrapped::Driver(self))} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } (x==std::any::TypeId::of::<dyn $crate::driver::Driver>()).then_some($crate::wrapper::StandardUnwrappedMut::Driver(self)) }  } };
    (@emit [$($g:tt)*] $t:ty => ConnectionFactory) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {(x==std::any::TypeId::of::<dyn $crate::driver::ConnectionFactory>()).then_some($crate::wrapper::StandardUnwrapped::ConnectionFactory(self))} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { if x == std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>() { return Some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)); } (x==std::any::TypeId::of::<dyn $crate::driver::ConnectionFactory>()).then_some($crate::wrapper::StandardUnwrappedMut::ConnectionFactory(self)) }  } };
    (@emit [$($g:tt)*] $t:ty => Wrapper) => { impl $($g)* $crate::wrapper::Wrapper for $t { fn wrapper_view(&self) -> &dyn $crate::wrapper::Wrapper { self } fn canonical_unwrap(&self,x:std::any::TypeId)->Option<$crate::wrapper::StandardUnwrapped<'_>> {(x==std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>()).then_some($crate::wrapper::StandardUnwrapped::Wrapper(self))} fn canonical_unwrap_mut(&mut self, x: std::any::TypeId) -> Option<$crate::wrapper::StandardUnwrappedMut<'_>> { (x==std::any::TypeId::of::<dyn $crate::wrapper::Wrapper>()).then_some($crate::wrapper::StandardUnwrappedMut::Wrapper(self)) }  } };
}
