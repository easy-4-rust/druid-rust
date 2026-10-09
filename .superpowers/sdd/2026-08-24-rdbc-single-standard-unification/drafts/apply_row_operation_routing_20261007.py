from pathlib import Path

root = Path.cwd()
p = root / 'crates/druid/src/core/filter_chain_impl.rs'
s = p.read_text()
marker = '    /// 按注册顺序包围属性调用；'
assert s.count(marker) == 1
bridge = '''    /// 按注册顺序包围原行操作；参数为上下文、操作及惰性驱动末端，返回原结果。
    /// 对应 Java: FilterChainImpl 的七个 ResultSet 行操作。
    pub(crate) fn standard_result_set_row_operation(
        &self,
        context: &ResultSetFilterContext,
        operation: super::ResultSetRowOperation,
        terminal: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            operation: super::ResultSetRowOperation,
            terminal: &mut dyn FnMut() -> Result<(), DruidError>,
        ) -> Result<(), DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter.standard_result_set_row_operation(
                    context, operation, &mut || invoke(remaining, context, operation, terminal),
                ),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, operation, terminal)
    }

'''
s = s.replace(marker, bridge + marker)
p.write_text(s, encoding='utf-8')
p = root / 'crates/druid/src/core/druid_pooled_result_set.rs'
s = p.read_text()
marker = '    /// 为已分类的结果集操作记录创建语句失败'
assert s.count(marker) == 1
helper = '''    /// 在同一资源锁内治理并包围行操作；参数为操作身份，返回原Filter或驱动结果。
    /// 对应 Java: DruidPooledResultSet 的七个行操作，旧租约/关闭先于Filter和驱动。
    fn filtered_row_operation(&self, operation: super::ResultSetRowOperation) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let mut terminal = || {
                match operation {
                    super::ResultSetRowOperation::InsertRow => raw.insert_row(),
                    super::ResultSetRowOperation::UpdateRow => raw.update_row(),
                    super::ResultSetRowOperation::DeleteRow => raw.delete_row(),
                    super::ResultSetRowOperation::RefreshRow => raw.refresh_row(),
                    super::ResultSetRowOperation::CancelRowUpdates => raw.cancel_row_updates(),
                    super::ResultSetRowOperation::MoveToInsertRow => raw.move_to_insert_row(),
                    super::ResultSetRowOperation::MoveToCurrentRow => raw.move_to_current_row(),
                }.map_err(super::DruidError::from)
            };
            match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_row_operation(
                    &self.filter_context, operation, &mut terminal,
                ).map_err(RdbcError::from),
                None => terminal().map_err(RdbcError::from),
            }
        });
        // access_core 已执行既有错误分类；结果集及原创建语句只计错一次。
        self.record_operation_result(result)
    }

'''
s = s.replace(marker, helper + marker)
for method, variant in [('insert_row','InsertRow'),('update_row','UpdateRow'),('delete_row','DeleteRow'),('refresh_row','RefreshRow'),('cancel_row_updates','CancelRowUpdates'),('move_to_insert_row','MoveToInsertRow'),('move_to_current_row','MoveToCurrentRow')]:
    old = f'''    fn {method}(&mut self) -> Result<(), RdbcError> {{
        self.ensure_open()?;
        self.record_operation_result(self.resource.access_core(ResultSet::{method}))
    }}'''
    assert s.count(old) == 1, method
    s = s.replace(old, f'''    fn {method}(&mut self) -> Result<(), RdbcError> {{
        self.filtered_row_operation(super::ResultSetRowOperation::{variant})
    }}''')
p.write_text(s, encoding='utf-8')
print('ROW_OPERATIONS_ROUTED_THROUGH_SAME_FILTER_AND_RESOURCE_POLICY')
