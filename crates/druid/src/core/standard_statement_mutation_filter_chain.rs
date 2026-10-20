use super::{BeforeFilter, DruidError, StatementMutation};
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::Statement;
use std::sync::Arc;

/// 语句设置与取消的有序包围链，返回最终生效参数供池化代理提交缓存。
/// 对应 Java: FilterChainImpl 的 statement_setXxx 和 statement_cancel。
pub struct StandardStatementMutationFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    statement: &'a mut dyn Statement,
    original: StatementMutation,
    applied: Option<StatementMutation>,
}

impl<'a> StandardStatementMutationFilterChain<'a> {
    /// 构造调用链；参数为过滤器、标准语句和原始操作，返回首项调用位置。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        statement: &'a mut dyn Statement,
        original: StatementMutation,
    ) -> Self {
        Self {
            filters,
            position: 0,
            statement,
            original,
            applied: None,
        }
    }

    /// 继续同一操作，可改写其参数；返回最终生效操作，原始驱动或过滤器错误原样传播。
    /// 对应 Java: FilterChain#statement_setXxx；禁止把设置操作替换成另一种操作。
    pub fn apply(&mut self, mutation: StatementMutation) -> Result<StatementMutation, DruidError> {
        if std::mem::discriminant(&mutation) != std::mem::discriminant(&self.original) {
            return Err(DruidError::InvalidArgument(
                "statement mutation kind changed".to_owned(),
            ));
        }
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            let effective = filter.standard_statement_mutation(self, mutation)?;
            if std::mem::discriminant(&effective) != std::mem::discriminant(&self.original) {
                return Err(DruidError::InvalidArgument(
                    "statement mutation kind changed".to_owned(),
                ));
            }
            Self::validate(&effective)?;
            if self
                .applied
                .as_ref()
                .is_some_and(|applied| applied != &effective)
            {
                return Err(DruidError::InvalidArgument(
                    "filter result differs from applied statement parameters".to_owned(),
                ));
            }
            return Ok(effective);
        }
        // 对应 Java FilterChain：先允许过滤器拒绝或改写，进入驱动前再校验最终参数。
        Self::validate(&mutation)?;
        let (operation, result) = match &mutation {
            StatementMutation::SetMaxFieldSize(value) => (
                "statement_set_max_field_size",
                self.statement.set_max_field_size(*value),
            ),
            StatementMutation::SetMaxRows(value) => (
                "statement_set_max_rows",
                self.statement.set_max_rows(*value),
            ),
            StatementMutation::SetEscapeProcessing(value) => (
                "statement_set_escape_processing",
                self.statement.set_escape_processing(*value),
            ),
            StatementMutation::SetQueryTimeout(value) => (
                "statement_set_query_timeout",
                self.statement.set_query_timeout(*value),
            ),
            StatementMutation::SetCursorName(value) => (
                "statement_set_cursor_name",
                self.statement.set_cursor_name(value),
            ),
            StatementMutation::SetCursorNameNullable(value) => (
                "statement_set_cursor_name_nullable",
                self.statement.set_cursor_name_nullable(value.as_deref()),
            ),
            StatementMutation::SetFetchDirection(value) => (
                "statement_set_fetch_direction",
                self.statement.set_fetch_direction(*value),
            ),
            StatementMutation::SetFetchSize(value) => (
                "statement_set_fetch_size",
                self.statement.set_fetch_size(*value),
            ),
            StatementMutation::SetPoolable(value) => (
                "statement_set_poolable",
                self.statement.set_poolable(*value),
            ),
            StatementMutation::CloseOnCompletion => (
                "statement_close_on_completion",
                self.statement.close_on_completion(),
            ),
            StatementMutation::Cancel => ("statement_cancel", self.statement.cancel()),
        };
        // 仅驱动明确不支持某个可缓存 setter 时保留既有回退；过滤器错误不经过此处。
        match result {
            Ok(()) => {
                self.applied = Some(mutation.clone());
                Ok(mutation)
            }
            Err(RdbcError::UnsupportedOperation { operation: actual })
                if (actual == operation
                    || (matches!(mutation, StatementMutation::SetCursorNameNullable(Some(_)))
                        && actual == "statement_set_cursor_name"))
                    && !matches!(
                        mutation,
                        StatementMutation::Cancel | StatementMutation::SetCursorNameNullable(None)
                    ) =>
            {
                self.applied = Some(mutation.clone());
                Ok(mutation)
            }
            Err(error) => Err(DruidError::from(error)),
        }
    }

    fn validate(mutation: &StatementMutation) -> Result<(), DruidError> {
        let invalid_name = match mutation {
            StatementMutation::SetMaxRows(value) if *value < 0 => Some("max rows"),
            StatementMutation::SetMaxFieldSize(value) if *value < 0 => Some("max field size"),
            StatementMutation::SetQueryTimeout(value) if *value < 0 => Some("query timeout"),
            StatementMutation::SetFetchSize(value) if *value < 0 => Some("fetch size"),
            _ => None,
        };
        if let Some(name) = invalid_name {
            Err(DruidError::InvalidArgument(format!(
                "{name} must be non-negative"
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod nullable_tests {
    use super::StandardStatementMutationFilterChain;
    use crate::core::StatementMutation;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::statement::Statement;

    struct UnsupportedNullable;
    druid_rdbc::impl_standard_wrapper!(UnsupportedNullable => Statement);

    #[async_trait::async_trait]
    impl Statement for UnsupportedNullable {
        fn set_cursor_name_nullable(&mut self, _name: Option<&str>) -> Result<(), RdbcError> {
            Err(RdbcError::UnsupportedOperation {
                operation: "statement_set_cursor_name_nullable",
            })
        }
    }

    #[test]
    fn null_cursor_unsupported_cannot_take_cached_property_success_fallback() {
        let mut driver = UnsupportedNullable;
        let mutation = StatementMutation::SetCursorNameNullable(None);
        let error = StandardStatementMutationFilterChain::new(&[], &mut driver, mutation.clone())
            .apply(mutation)
            .unwrap_err();
        assert_eq!(
            RdbcError::from(error),
            RdbcError::UnsupportedOperation {
                operation: "statement_set_cursor_name_nullable"
            }
        );
    }
}
