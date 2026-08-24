use crate::error::RdbcError;
use crate::wrapper::StandardUnwrapped;
use std::any::{Any, TypeId};

/// JDBC `Wrapper` 的 object-safe Rust 标准。
///
/// 泛型便捷方法位于 [`WrapperExt`]，避免破坏 dyn compatibility。`None` 精确保留 Java
/// `null Class<?>` 输入，并返回 [`RdbcError::InvalidArgument`]；unsupported target 不会被
/// `Option::None` 吞掉。
pub trait Wrapper: Any + Send {
    /// 返回具体运行时对象视图。
    fn as_any(&self) -> &dyn Any;

    /// 判断对象能否解包为给定运行时类型。
    ///
    /// `None` 返回 [`RdbcError::InvalidArgument`]；unknown target 返回 `Ok(false)`。
    fn is_wrapper_for(&self, target: Option<TypeId>) -> Result<bool, RdbcError> {
        let target = require_wrapper_target(target)?;
        Ok(self.as_any().type_id() == target)
    }

    /// 返回 object-safe 标准解包结果。
    ///
    /// `None` 返回 [`RdbcError::InvalidArgument`]；unsupported target 返回 typed
    /// `SQLFeatureNotSupportedException` mapping。
    fn unwrap(&self, target: Option<TypeId>) -> Result<StandardUnwrapped<'_>, RdbcError> {
        let target = require_wrapper_target(target)?;
        if self.as_any().type_id() == target {
            return Ok(StandardUnwrapped::Object(self.as_any()));
        }
        Err(RdbcError::feature_not_supported("wrapper_unwrap_target"))
    }
}

fn require_wrapper_target(target: Option<TypeId>) -> Result<TypeId, RdbcError> {
    target.ok_or_else(|| RdbcError::InvalidArgument("wrapper target must not be null".to_string()))
}

/// [`Wrapper`] 的强类型 concrete-object 扩展。
pub trait WrapperExt: Wrapper {
    /// 判断对象能否解包为具体 `T`。
    fn is_wrapper_for_type<T: Any>(&self) -> Result<bool, RdbcError> {
        self.is_wrapper_for(Some(TypeId::of::<T>()))
    }

    /// 尝试把对象解包为具体 `T` 的只读引用。
    ///
    /// Canonical trait-object variants 不在这里伪装成具体 [`Any`] 对象。
    fn unwrap_ref<T: Any>(&self) -> Result<&T, RdbcError> {
        let unwrapped = self.unwrap(Some(TypeId::of::<T>()))?;
        unwrapped
            .object()
            .and_then(|object| object.downcast_ref::<T>())
            .ok_or_else(|| {
                RdbcError::InvalidArgument(
                    "wrapper target resolved to a canonical interface, not a concrete object"
                        .to_string(),
                )
            })
    }
}

impl<T> WrapperExt for T where T: Wrapper + ?Sized {}
