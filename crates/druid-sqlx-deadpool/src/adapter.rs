//! 对应 Java 类：DruidDataSource（sqlx-deadpool adapter）

pub struct SqlxDeadpoolAdapter {
    _placeholder: (),
}

impl SqlxDeadpoolAdapter {
    pub fn new() -> Self { Self { _placeholder: () } }
}

impl Default for SqlxDeadpoolAdapter {
    fn default() -> Self { Self::new() }
}
