//! 标准对象的运行时识别与强类型解包。

mod standard_unwrapped;
mod standard_unwrapped_mut;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod wrapper;

pub use standard_unwrapped::StandardUnwrapped;
pub use standard_unwrapped_mut::StandardUnwrappedMut;
pub use wrapper::{Wrapper, WrapperExt};
