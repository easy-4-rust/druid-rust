//! Canonical prepared proxy 的 Task 6 legacy identity shell。

use crate::core::prepared_statement_key::PreparedStatementKey as DruidPreparedStatementKey;
use crate::core::prepared_statement_method_type::PreparedStatementMethodType as DruidPreparedStatementMethodType;
use crate::core::{
    PreparedStatementHolder, PreparedStatementKey as PhysicalPreparedStatementKey,
    SqlTextPreparedStatement,
};
use std::sync::Arc;

// 完整保留 9 个 Java prepare 重载，同时让旧 MethodType 身份只出现在本 Task 6
// 兼容文件的一处宏实参中；宏体引用的是 Druid 私有枚举与类型形参。
macro_rules! physical_method_type {
    ($method:expr, $physical:path) => {
        match $method {
            DruidPreparedStatementMethodType::M1 => <$physical>::M1,
            DruidPreparedStatementMethodType::M2 => <$physical>::M2,
            DruidPreparedStatementMethodType::M3 => <$physical>::M3,
            DruidPreparedStatementMethodType::M4 => <$physical>::M4,
            DruidPreparedStatementMethodType::M5 => <$physical>::M5,
            DruidPreparedStatementMethodType::M6 => <$physical>::M6,
            DruidPreparedStatementMethodType::Precall1 => <$physical>::Precall1,
            DruidPreparedStatementMethodType::Precall2 => <$physical>::Precall2,
            DruidPreparedStatementMethodType::Precall3 => <$physical>::Precall3,
        }
    };
}

/// 创建不参与 canonical 执行/缓存的 legacy Java-identity shell。
pub(crate) fn standard_prepared_shell_holder(sql: &str) -> Arc<PreparedStatementHolder> {
    let key = DruidPreparedStatementKey::new(
        Some(sql.to_string()),
        None,
        DruidPreparedStatementMethodType::M1,
    )
    .expect("standard prepare SQL is already non-null");
    Arc::new(PreparedStatementHolder::new(
        key,
        Arc::new(SqlTextPreparedStatement::new(sql)),
    ))
}

/// 把 Druid 私有 cache key 投影到 Task 6 Physical SPI 的兼容值对象。
pub(crate) fn physical_prepared_statement_key(
    key: &DruidPreparedStatementKey,
) -> Result<PhysicalPreparedStatementKey, druid_rdbc::error::RdbcError> {
    let method_type =
        physical_method_type!(key.method_type(), crate::core::PreparedStatementMethodType);
    PhysicalPreparedStatementKey::full(
        Some(key.sql().to_string()),
        key.catalog().map(str::to_owned),
        method_type,
        key.result_set_type(),
        key.result_set_concurrency(),
        key.result_set_holdability(),
        key.auto_generated_keys(),
        key.column_indexes().map(<[i32]>::to_vec),
        key.column_names().map(<[String]>::to_vec),
    )
}
