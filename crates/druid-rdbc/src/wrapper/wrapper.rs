use crate::error::RdbcError;
use crate::wrapper::StandardUnwrapped;
use std::any::{Any, TypeId};

/// JDBC `Wrapper` 的 object-safe canonical identity 标准。
pub trait Wrapper: Send {
    /// 返回实现者唯一的 canonical interface view。
    fn canonical_unwrap(&self) -> StandardUnwrapped<'_>;

    /// 判断对象是否实现目标 canonical interface。
    fn is_wrapper_for(&self, target: Option<TypeId>) -> Result<bool, RdbcError> {
        let target = require_wrapper_target(target)?;
        Ok(self.canonical_unwrap().type_id() == target)
    }

    /// 解包为目标 canonical interface。
    fn unwrap(&self, target: Option<TypeId>) -> Result<StandardUnwrapped<'_>, RdbcError> {
        let target = require_wrapper_target(target)?;
        let view = self.canonical_unwrap();
        if view.type_id() == target {
            return Ok(view);
        }
        Err(RdbcError::feature_not_supported("wrapper_unwrap_target"))
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

/// 为一个 external implementor 生成单一 canonical Wrapper view。
#[macro_export]
macro_rules! impl_standard_wrapper {
    ($target:ty => Connection) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::Connection(self)
            }
        }
    };
    ($target:ty => Statement) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::Statement(self)
            }
        }
    };
    ($target:ty => ResultSet) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::ResultSet(self)
            }
        }
    };
    ($target:ty => ResultSetMetaData) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::ResultSetMetaData(self)
            }
        }
    };
    ($target:ty => PreparedStatement) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::PreparedStatement(self)
            }
        }
    };
    ($target:ty => DatabaseMetaData) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::DatabaseMetaData(self)
            }
        }
    };
    ($target:ty => DataSource) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::DataSource(self)
            }
        }
    };
    ($target:ty => ParameterMetaData) => {
        impl $crate::wrapper::Wrapper for $target {
            fn canonical_unwrap(&self) -> $crate::wrapper::StandardUnwrapped<'_> {
                $crate::wrapper::StandardUnwrapped::ParameterMetaData(self)
            }
        }
    };
}
