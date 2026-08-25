//! Task 6 Physical lease 方法签名的旧 key 类型边界。

// 宏让调用模块不重新导入旧公共类型；Task 6 删除 Physical SPI 时整文件删除。
macro_rules! physical_prepared_statement_key_type {
    () => {
        $crate::core::PreparedStatementKey
    };
}

pub(crate) use physical_prepared_statement_key_type;
