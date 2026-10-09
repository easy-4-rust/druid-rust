from pathlib import Path
r=Path(".")
p=r/"crates/druid/src/core/standard_result_set_filter_chain.rs"
s=p.read_text(encoding="utf-8").replace("use super::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};","use super::{DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetObjectRead, Value};",1)
method="""    /// 依注册顺序包围完整对象读取；参数保留原列名、Map/Class引用，返回原对象。
    /// 对应 Java: FilterChainImpl#resultSet_getObject 的全部六种重载。
    pub(crate) fn result_set_object(
        &mut self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetObjectRead<'_>,
    ) -> Result<RdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            let context = self.context;
            return filter.standard_result_set_object(context, column, label, request, &mut || {
                self.result_set_object(column, label, request)
            });
        }
        // 默认末端直接调用同一原生重载，不能定位虚拟列名或把资源对象转成标量。
        let index = || column.ok_or_else(|| druid_rdbc::error::RdbcError::InvalidArgument(
            "object read requires a column or label".to_owned(),
        ));
        let result = match (label, request) {
            (None, ResultSetObjectRead::Ordinary) => self.standard.object(index()?),
            (Some(label), ResultSetObjectRead::Ordinary) => self.standard.object_by_label(label),
            (None, ResultSetObjectRead::TypeMap(map)) => self.standard.object_with_type_map(index()?, *map),
            (Some(label), ResultSetObjectRead::TypeMap(map)) => self.standard.object_with_type_map_by_label(label, *map),
            (None, ResultSetObjectRead::Typed(target)) => self.standard.object_as(index()?, target),
            (Some(label), ResultSetObjectRead::Typed(target)) => self.standard.object_as_by_label(label, target),
        };
        result.map_err(DruidError::from)
    }

"""
s=s.replace("    pub(crate) async fn result_set_next",method+"    pub(crate) async fn result_set_next",1);p.write_text(s,encoding="utf-8")
p=r/"crates/druid/src/core/filter_chain_impl.rs";s=p.read_text(encoding="utf-8")
method="""    /// 从首个Filter包围完整对象重载，返回未扁平化的驱动或短路对象。
    /// 参数为驱动、上下文、治理列、原请求和原列名；对应 Java: FilterChainImpl#getObject。
    pub(crate) fn standard_result_set_object(
        &self,
        result_set: &mut dyn druid_rdbc::result_set::ResultSet,
        context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        request: &super::ResultSetObjectRead<'_>,
        label: Option<&str>,
    ) -> Result<druid_rdbc::types::RdbcObject, DruidError> {
        StandardResultSetFilterChain::new(&self.result_set, result_set, context)
            .result_set_object(column, label, request)
    }

"""
s=s.replace("    pub(crate) fn standard_result_set_typed_value",method+"    pub(crate) fn standard_result_set_typed_value",1);p.write_text(s,encoding="utf-8")
p=r/"crates/druid/src/core/druid_pooled_result_set.rs";s=p.read_text(encoding="utf-8")
helper="""    /// 在同一资源锁内先治理列访问再运行对象过滤链，只向原创建语句记错一次。
    /// 参数保留六种原重载身份；返回原资源对象或精确错误。
    /// 对应 Java: DruidPooledResultSet#getObject(int/String, Map/Class)。
    fn filtered_object(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &super::ResultSetObjectRead<'_>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        let result = self.resource.access_core(|raw| {
            let column = if let Some(label) = label {
                self.validate_object_label(raw, label)?;
                None
            } else {
                column.map(|column| self.driver_column_index(column)).transpose()?
            };
            match &self.resource.filter_chain {
                Some(filters) => filters.standard_result_set_object(
                    raw, &self.filter_context, column, request, label,
                ),
                None => super::StandardResultSetFilterChain::new(&[], raw, &self.filter_context)
                    .result_set_object(column, label, request),
            }.map_err(RdbcError::from)
        });
        if result.is_err() {
            // access_core已按租约策略分类，创建语句只记录本次读取失败一次。
            self.statement.record_exception();
        }
        result
    }

"""
s=s.replace("    fn validate_object_label",helper+"    fn validate_object_label",1)
replacements={
"object":"        self.filtered_object(Some(column), None, &super::ResultSetObjectRead::Ordinary)\n",
"object_by_label":"        self.filtered_object(None, Some(label), &super::ResultSetObjectRead::Ordinary)\n",
"object_with_type_map":"        self.filtered_object(Some(column), None, &super::ResultSetObjectRead::TypeMap(type_map))\n",
"object_with_type_map_by_label":"        self.filtered_object(None, Some(label), &super::ResultSetObjectRead::TypeMap(type_map))\n",
"object_as":"        self.filtered_object(Some(column), None, &super::ResultSetObjectRead::Typed(target_type))\n",
"object_as_by_label":"        self.filtered_object(None, Some(label), &super::ResultSetObjectRead::Typed(target_type))\n",
}
for name,body in replacements.items():
 start=s.index(f"    fn {name}(")
 opening=s.index("{",start);depth=1;i=opening+1
 while depth:
  if s[i]=="{":depth+=1
  elif s[i]=="}":depth-=1
  i+=1
 s=s[:opening+1]+"\n"+body+"    "+s[i-1:]
p.write_text(s,encoding="utf-8")
print("Routed all six owned object overloads through ordered full-object chain with common governance.")
