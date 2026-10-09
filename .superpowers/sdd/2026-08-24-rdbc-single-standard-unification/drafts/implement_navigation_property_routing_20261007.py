from pathlib import Path
import re

root = Path(__file__).resolve().parents[4]
core = root / 'crates/druid/src/core'
def change(path, transform):
    old = path.read_text(encoding='utf-8')
    new = transform(old)
    assert new != old, path
    path.write_text(new, encoding='utf-8', newline='\n')
def replace(s, old, new):
    assert s.count(old) == 1, old[:100]
    return s.replace(old, new, 1)

value = core / 'result_set_property_value.rs'
methods = [('boolean','Boolean','bool'),('integer','Integer','i32'),('direction','FetchDirection','FetchDirection'),('result_set_type','ResultSetType','ResultSetType'),('concurrency','Concurrency','ResultSetConcurrency'),('holdability','Holdability','ResultSetHoldability'),('string','String','Option<String>'),('column','Column','ColumnIndex')]
body = '\nimpl ResultSetPropertyValue {\n'
for name, variant, typ in methods:
    body += f'''    /// 提取{variant}属性；无参数，类型不匹配时返回标准参数错误。
    pub(crate) fn into_{name}(self) -> Result<{typ}, RdbcError> {{
        match self {{
            Self::{variant}(value) => Ok(value),
            _ => Err(RdbcError::InvalidArgument("incompatible result set property filter value".to_owned())),
        }}
    }}
'''
body += '''    /// 提取 setter 完成状态；无参数，拒绝不兼容的 Filter 返回类型。
    pub(crate) fn into_unit(self) -> Result<(), RdbcError> {
        match self {
            Self::Unit => Ok(()),
            _ => Err(RdbcError::InvalidArgument("incompatible result set property filter value".to_owned())),
        }
    }
}
'''
change(value, lambda s: s + body)

chain_methods = '''    /// 包围导航原参数；仅驱动末端执行时取得实际行号，短路不访问游标。
    /// 对应 Java: FilterChainImpl#resultSet_previous/absolute/relative。
    pub(crate) async fn standard_result_set_navigation(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<(bool, Option<i32>), DruidError> {
        let mut chain = StandardResultSetFilterChain::new(&self.result_set, result_set, context);
        let present = chain.result_set_navigation(navigation).await?;
        Ok((present, chain.navigation_row()))
    }

    /// 按注册顺序包围属性调用；参数为原请求和末端，保留短路及错误反向退出。
    /// 对应 Java: FilterChainImpl 的 ResultSet 属性方法。
    pub(crate) fn standard_result_set_property(
        &self,
        context: &ResultSetFilterContext,
        request: super::ResultSetPropertyOperation<'_>,
        terminal: &mut dyn FnMut() -> Result<super::ResultSetPropertyValue, DruidError>,
    ) -> Result<super::ResultSetPropertyValue, DruidError> {
        fn invoke(
            filters: &[Arc<dyn super::ResultSetFilter>],
            context: &ResultSetFilterContext,
            request: super::ResultSetPropertyOperation<'_>,
            terminal: &mut dyn FnMut() -> Result<super::ResultSetPropertyValue, DruidError>,
        ) -> Result<super::ResultSetPropertyValue, DruidError> {
            match filters.split_first() {
                Some((filter, remaining)) => filter.standard_result_set_property(context, request,
                    &mut || invoke(remaining, context, request, terminal)),
                None => terminal(),
            }
        }
        invoke(&self.result_set, context, request, terminal)
    }

'''
change(core/'filter_chain_impl.rs', lambda s: replace(s, '    pub(crate) async fn standard_result_set_next(', chain_methods+'    pub(crate) async fn standard_result_set_next('))

nav = '''    /// 依注册顺序惰性包围导航；短路不会轮询下游 future 或读取原生行号。
    /// 参数为原导航描述符，返回原生或 Filter 的布尔结果。
    pub(crate) async fn result_set_navigation(
        &mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            let context = self.context;
            filter.standard_result_set_navigation(context, navigation,
                Box::pin(self.result_set_navigation(navigation))).await
        } else {
            let present = self.standard.navigate(navigation).await.map_err(DruidError::from)?;
            self.navigation_row = Some(self.standard.row().map_err(DruidError::from)?);
            Ok(present)
        }
    }

    /// 返回本次导航末端实际行号；无参数，短路时为 None。
    pub(crate) const fn navigation_row(&self) -> Option<i32> {
        self.navigation_row
    }

'''
def update_chain(s):
    s = replace(s, '    position: usize,', '    position: usize,\n    navigation_row: Option<i32>,')
    s = replace(s, '            position: 0,', '            position: 0,\n            navigation_row: None,')
    return replace(s, '    pub(crate) async fn result_set_next(', nav+'    pub(crate) async fn result_set_next(')
change(core/'standard_result_set_filter_chain.rs', update_chain)

def update_access(s):
    s = replace(s, ') -> Result<bool, RdbcError> {', ') -> Result<(bool, Option<i32>), RdbcError> {')
    return replace(s, '        let result = raw.navigate(navigation).await;', '''        // 等待锁期间租约可能结束；末端与短路都必须服从实际租约。
        self.policy.ensure_open()?;
        let result = match &self.filter_chain {
            Some(filters) => filters.standard_result_set_navigation(raw.as_mut(), &self.filter_context, navigation)
                .await.map_err(RdbcError::from),
            None => match raw.navigate(navigation).await {
                Ok(present) => raw.row().map(|row| (present, Some(row))),
                Err(error) => Err(error),
            },
        };''')
change(core/'druid_pooled_result_set/access.rs', update_access)

helper = '''    /// 在同一资源锁内包围属性并校验返回类型，分类与原创建语句计错各一次。
    /// 参数为原请求、原生末端和类型提取；返回属性值或原错误。
    /// 对应 Java: DruidPooledResultSet 的属性 Filter 分派。
    fn filtered_property<T>(
        &self,
        request: super::ResultSetPropertyOperation<'_>,
        mut terminal: impl FnMut(&mut dyn ResultSet) -> Result<super::ResultSetPropertyValue, RdbcError>,
        decode: impl FnOnce(super::ResultSetPropertyValue) -> Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let value = match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_property(&self.filter_context, request,
                    &mut || terminal(raw).map_err(crate::core::DruidError::from))
                    .map_err(RdbcError::from)?,
                None => terminal(raw)?,
            };
            decode(value)
        });
        self.record_operation_result(result)
    }

'''
def update_pooled(s):
    s = replace(s, '    fn filtered_object(', helper+'    fn filtered_object(')
    s = replace(s, '''        let present =
            self.record_operation_result(self.resource.navigate_core(navigation).await)?;
        let row = self.record_operation_result(self.resource.access_core(|raw| raw.row()))?;
        self.shared.cursor_index.store(row, Ordering::Release);
        if present {
            self.filter_context.record_fetch_row_count(row);
        }''', '''        let (present, native_row) =
            self.record_operation_result(self.resource.navigate_core(navigation).await)?;
        if let Some(row) = native_row {
            self.shared.cursor_index.store(row, Ordering::Release);
            if present {
                self.filter_context.record_fetch_row_count(row);
            }
        }''')
    mappings = [('row','Row','Integer','integer'),('is_before_first','IsBeforeFirst','Boolean','boolean'),('is_after_last','IsAfterLast','Boolean','boolean'),('is_first','IsFirst','Boolean','boolean'),('is_last','IsLast','Boolean','boolean'),('fetch_direction','FetchDirection','FetchDirection','direction'),('fetch_size','FetchSize','Integer','integer'),('result_set_type','ResultSetType','ResultSetType','result_set_type'),('concurrency','Concurrency','Concurrency','concurrency'),('holdability','Holdability','Holdability','holdability'),('cursor_name','CursorName','String','string'),('row_updated','RowUpdated','Boolean','boolean'),('row_inserted','RowInserted','Boolean','boolean'),('row_deleted','RowDeleted','Boolean','boolean')]
    for method, op, val, decoder in mappings:
        old = f'        self.ensure_open()?;\n        self.record_operation_result(self.resource.access_core(|raw| raw.{method}()))'
        new = f'''        self.filtered_property(super::ResultSetPropertyOperation::{op},
            |raw| raw.{method}().map(super::ResultSetPropertyValue::{val}),
            super::ResultSetPropertyValue::into_{decoder})'''
        s = replace(s, old, new)
    s = replace(s, '''        self.ensure_open()?;
        let driver = self
            .resource
            .access_core(|raw| raw.find_column(column_label))?;
        self.logical_column_index(driver)''', '''        self.filtered_property(super::ResultSetPropertyOperation::FindColumn(column_label),
            |raw| raw.find_column(column_label).map(super::ResultSetPropertyValue::Column),
            |value| self.logical_column_index(value.into_column()?))''')
    for method, op, arg in [('set_fetch_direction','SetFetchDirection','direction'),('set_fetch_size','SetFetchSize','rows')]:
        pattern = r'        self.ensure_open\(\)\?;\n        self.record_operation_result\(\s*self.resource\s*\.access_core\(\|raw\| raw\.'+method+r'\('+arg+r'\)\),?\s*\)'
        s, count = re.subn(pattern, f'''        self.filtered_property(super::ResultSetPropertyOperation::{op}({arg}),
            |raw| raw.{method}({arg}).map(|()| super::ResultSetPropertyValue::Unit),
            super::ResultSetPropertyValue::into_unit)''', s)
        assert count == 1, method
    s = replace(s, '''        self.ensure_open()?;
        self.resource.was_null_core()''', '''        self.filtered_property(super::ResultSetPropertyOperation::WasNull,
            |raw| {
                // 默认属性委托保留原 wasNull before/after 链，不能绕过已有观察。
                let value = match &self.resource.filter_chain {
                    Some(filters) => filters.standard_result_set_was_null(raw, &self.filter_context)
                        .map_err(RdbcError::from),
                    None => raw.was_null(),
                }?;
                Ok(super::ResultSetPropertyValue::Boolean(value))
            }, super::ResultSetPropertyValue::into_boolean)''')
    # 关闭状态是本地快照，关闭后或租约失效时保留既有可观察性。
    s = replace(s, '''        Ok(self.shared.closed.load(Ordering::Acquire) || self.resource.is_resource_closed())''', '''        let closed = self.shared.closed.load(Ordering::Acquire) || self.resource.is_resource_closed();
        if closed || !self.statement.inner.lease_active.load(Ordering::Acquire) {
            return Ok(closed);
        }
        self.filtered_property(super::ResultSetPropertyOperation::IsClosed,
            |_| Ok(super::ResultSetPropertyValue::Boolean(closed)),
            super::ResultSetPropertyValue::into_boolean)''')
    # 旧私有函数已并入同一个分类闭包，避免遗留无调用入口。
    start = s.index('    fn was_null_core(')
    end = s.index('\n}\n', start)
    s = s[:start] + s[end:]
    return s
change(core/'druid_pooled_result_set.rs', update_pooled)
print('navigation/property routing applied; no ledger binding changed')
