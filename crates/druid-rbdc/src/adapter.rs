//! 对应 Java 类：DruidDataSource（rbdc adapter）
//!
//! rbdc connection adapter（planned for V2）。

pub struct RbdcAdapter {
    _placeholder: (),
}

impl RbdcAdapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for RbdcAdapter {
    fn default() -> Self { Self::new() }
}
