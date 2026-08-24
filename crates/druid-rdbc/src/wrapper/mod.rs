//! 标准对象的运行时识别与强类型解包。

mod standard_unwrapped;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod wrapper;

pub use standard_unwrapped::StandardUnwrapped;
pub use wrapper::{Wrapper, WrapperExt};
