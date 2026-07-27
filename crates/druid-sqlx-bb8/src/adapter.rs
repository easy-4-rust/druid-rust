//! 对应 Java 类：DruidDataSource（sqlx-bb8 adapter）

pub struct SqlxBb8Adapter {
    _placeholder: (),
}

impl SqlxBb8Adapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for SqlxBb8Adapter {
    fn default() -> Self { Self::new() }
}
