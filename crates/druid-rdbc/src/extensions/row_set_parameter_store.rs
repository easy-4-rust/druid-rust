use super::row_set_binding_key::RowSetBindingKey;
use crate::error::{RdbcError, SqlException};
use crate::statement::{ParameterIndex, PreparedInputParameter};
use std::collections::BTreeMap;

/// RowSet 参数的无损状态缓冲；重用标准 setter 描述符而不提前转换流或对象。
/// 对应 Java: javax.sql.RowSet#setXxx/clearParameters、RowSetInternal#getParams 的参数状态部分。
/// 本 Rust 扩展不替代完整 RowSet；执行、事件及原始行由 RowSet 实现负责。
#[derive(Debug, Clone, Default)]
pub struct RowSetParameterStore {
    // 仅保存每个 selector 的最近一次绑定；顺序用于名称与位置解析后的最后写入胜出。
    bindings: Vec<(RowSetBindingKey, PreparedInputParameter)>,
}

impl RowSetParameterStore {
    /// 创建空参数状态，无参数；返回尚未绑定任何位置或名称的缓冲。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用已验证的 1-based 参数 index 保存完整 parameter，替换同一位置的旧绑定。
    /// 无返回值；保留 NULL、Calendar、长度重载及资源引用，不提前读取参数。
    pub fn set_index(&mut self, index: ParameterIndex, parameter: PreparedInputParameter) {
        self.set_binding(RowSetBindingKey::Index(index.get()), parameter);
    }

    /// 保存参数 name 的完整 parameter；null 或空名称返回 SQL 异常且不修改旧状态。
    /// 不修剪或改写名称；最终 SQL 位置必须由 provider 在执行前解析。
    pub fn set_named(
        &mut self,
        name: Option<&str>,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        let name = name.filter(|name| !name.is_empty()).ok_or_else(|| {
            Self::parameter_error("07009", "RowSet parameter name is null or empty")
        })?;
        self.set_binding(RowSetBindingKey::Name(name.to_owned()), parameter);
        Ok(())
    }

    /// 返回当前不同 selector 的数量，无参数；同一位置和其名称在解析前分别计数。
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// 返回是否尚无任何绑定，无参数；不会读取或解析参数。
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// 获取按 1-based 位置排列的非消费参数快照，无参数。
    /// 缺槽或存在未解析名称时返回 SQL 异常，不创造虚构位置或将缺槽变成 SQL NULL。
    /// 对应 Java: javax.sql.RowSetInternal#getParams 的位置参数部分。
    pub fn get_params(&self) -> Result<Vec<PreparedInputParameter>, RdbcError> {
        self.resolve_params(|_| {
            Err(Self::parameter_error(
                "07009",
                "RowSet named parameters require provider resolution",
            ))
        })
    }

    /// 使用参数 resolve 将每个原始名称解析到真实 SQL 位置，返回非消费参数快照。
    /// 同一位置的最后一次 setter 生效；解析错误原样传播，不修改自身参数状态。
    /// 缺槽在分配结果 Vec 前拒绝；驱动必须根据当前 command 解析，不能按名称排序猜测。
    pub fn resolve_params<F>(
        &self,
        mut resolve: F,
    ) -> Result<Vec<PreparedInputParameter>, RdbcError>
    where
        F: FnMut(&str) -> Result<ParameterIndex, RdbcError>,
    {
        let mut parameters = BTreeMap::new();
        for (key, parameter) in &self.bindings {
            let index = match key {
                RowSetBindingKey::Index(index) => *index,
                RowSetBindingKey::Name(name) => resolve(name)?.get(),
            };
            parameters.insert(index, parameter.clone());
        }
        // 正整数唯一键的最大值等于键数量，当且仅当位置恰好连续为 1..=len。
        // 先检查可避免一个极端稀疏 index 导致巨量分配，也不会用 NULL 掩盖未绑定参数。
        if parameters
            .last_key_value()
            .is_some_and(|(last, _)| *last != parameters.len())
        {
            return Err(Self::parameter_error(
                "07001",
                "RowSet has an unbound parameter position",
            ));
        }
        Ok(parameters.into_values().collect())
    }

    /// 清除所有参数并释放自身引用，无参数、无返回值；不隐式 close 调用者持有的流。
    /// 对应 Java: javax.sql.RowSet#clearParameters。
    pub fn clear_parameters(&mut self) {
        self.bindings.clear();
    }

    fn set_binding(&mut self, key: RowSetBindingKey, parameter: PreparedInputParameter) {
        self.bindings.retain(|(existing, _)| existing != &key);
        self.bindings.push((key, parameter));
    }

    fn parameter_error(state: &str, message: &str) -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(
            0,
            Some(state.to_owned()),
            Some(message.to_owned()),
        )))
    }
}
