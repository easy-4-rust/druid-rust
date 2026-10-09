//! Canonical async ResultSet 的 Druid around-chain。

use super::{DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetObjectRead, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead, ResultSetUpdate};
use druid_rdbc::types::RdbcObject;
use std::sync::Arc;

/// 让现有 ResultSetFilter 以相同注册顺序包围 async standard cursor。
pub(crate) struct StandardResultSetFilterChain<'a> {
    filters: &'a [Arc<dyn ResultSetFilter>],
    position: usize,
    navigation_row: Option<i32>,
    capture_navigation_row: bool,
    read_position: usize,
    string_read: bool,
    typed_read: Option<&'a ResultSetRead>,
    typed_label: Option<&'a str>,
    standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
    context: &'a ResultSetFilterContext,
}

impl<'a> StandardResultSetFilterChain<'a> {
    pub(crate) fn new(
        filters: &'a [Arc<dyn ResultSetFilter>],
        standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            navigation_row: None,
            capture_navigation_row: false,
            read_position: 0,
            string_read: false,
            typed_read: None,
            typed_label: None,
            standard,
            context,
        }
    }

    /// 创建池化游标所需的导航行号跟踪链；参数为原链、驱动和上下文，返回同一标准链。
    /// 普通默认委托不额外读取 row；池化路径仅在真实导航末端执行后取得实际行号。
    pub(crate) fn new_with_navigation_row(
        filters: &'a [Arc<dyn ResultSetFilter>],
        standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            capture_navigation_row: true,
            ..Self::new(filters, standard, context)
        }
    }

    /// 复用相同 around-chain，末端分派到驱动的字符串读取入口。
    pub(crate) fn new_string(
        filters: &'a [Arc<dyn ResultSetFilter>],
        standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            string_read: true,
            ..Self::new(filters, standard, context)
        }
    }

    /// 保留 around-chain，末端用完整请求和原列名分派强类型标量 getter。
    /// 参数为 Filter、驱动、上下文、请求和可空列名，返回链。
    /// 对应 Java: FilterChainImpl 的强类型 ResultSet getter。
    pub(crate) fn new_typed(
        filters: &'a [Arc<dyn ResultSetFilter>],
        standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
        context: &'a ResultSetFilterContext,
        request: &'a ResultSetRead,
        label: Option<&'a str>,
    ) -> Self {
        Self {
            typed_read: Some(request),
            typed_label: label,
            ..Self::new(filters, standard, context)
        }
    }

    /// 依注册顺序包围完整对象读取；参数保留原列名、Map/Class引用，返回原对象。
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
        let index = || {
            column.ok_or_else(|| {
                druid_rdbc::error::RdbcError::InvalidArgument(
                    "object read requires a column or label".to_owned(),
                )
            })
        };
        let result = match (label, request) {
            (None, ResultSetObjectRead::Ordinary) => self.standard.object(index()?),
            (Some(label), ResultSetObjectRead::Ordinary) => self.standard.object_by_label(label),
            (None, ResultSetObjectRead::TypeMap(map)) => {
                self.standard.object_with_type_map(index()?, *map)
            }
            (Some(label), ResultSetObjectRead::TypeMap(map)) => {
                self.standard.object_with_type_map_by_label(label, *map)
            }
            (None, ResultSetObjectRead::Typed(target)) => self.standard.object_as(index()?, target),
            (Some(label), ResultSetObjectRead::Typed(target)) => {
                self.standard.object_as_by_label(label, target)
            }
        };
        result.map_err(DruidError::from)
    }

    /// 按注册顺序包围资源对象读取；参数保留下标、原标签和请求，返回原句柄。
    /// 对应 Java: FilterChainImpl#resultSet_getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML。
    pub(crate) fn result_set_resource(
        &mut self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            let context = self.context;
            return filter.standard_result_set_resource(
                context,
                column,
                label,
                request,
                &mut || self.result_set_resource(column, label, request),
            );
        }
        let result = match label {
            Some(label) => self.standard.read_object_by_label(label, request),
            None => self.standard.read_object(
                column.ok_or_else(|| {
                    DruidError::InvalidArgument("resource read requires a column".to_owned())
                })?,
                request,
            ),
        };
        result.map_err(DruidError::from)
    }

    /// 按原下标或标签委托字节流，不消费或复制资源内容。
    /// 参数为列身份和流类型，返回同一可空流。对应 Java: FilterChainImpl#resultSet_getXxxStream。
    pub(crate) fn result_set_input_stream(
        &mut self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            let context = self.context;
            return filter.standard_result_set_input_stream(
                context,
                column,
                label,
                kind,
                &mut || self.result_set_input_stream(column, label, kind),
            );
        }
        let result = match label {
            Some(label) => self.standard.input_stream_by_label(label, kind),
            None => self.standard.input_stream(
                column.ok_or_else(|| {
                    DruidError::InvalidArgument("stream read requires a column".to_owned())
                })?,
                kind,
            ),
        };
        result.map_err(DruidError::from)
    }

    /// 按原重载委托字符流；参数保留列身份和national标志，返回原Reader。
    /// 对应 Java: FilterChainImpl#resultSet_getCharacterStream/getNCharacterStream。
    pub(crate) fn result_set_reader(
        &mut self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            let context = self.context;
            return filter.standard_result_set_reader(
                context,
                column,
                label,
                national,
                &mut || self.result_set_reader(column, label, national),
            );
        }
        let result = match label {
            Some(label) => self.standard.reader_by_label(label, national),
            None => self.standard.reader(
                column.ok_or_else(|| {
                    DruidError::InvalidArgument("reader read requires a column".to_owned())
                })?,
                national,
            ),
        };
        result.map_err(DruidError::from)
    }

    /// 依注册顺序惰性包围导航；短路不会轮询下游 future 或读取原生行号。
    /// 参数为原导航描述符，返回原生或 Filter 的布尔结果。
    pub(crate) fn result_set_navigation<'b>(
        &'b mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, DruidError>> + Send + 'b>>
    {
        Box::pin(async move {
            if self.position < self.filters.len() {
                let filter = Arc::clone(&self.filters[self.position]);
                self.position += 1;
                let context = self.context;
                filter
                    .standard_result_set_navigation(
                        context,
                        navigation,
                        self.result_set_navigation(navigation),
                    )
                    .await
            } else {
                let present = self
                    .standard
                    .navigate(navigation)
                    .await
                    .map_err(DruidError::from)?;
                if self.capture_navigation_row {
                    self.navigation_row = Some(self.standard.row().map_err(DruidError::from)?);
                }
                Ok(present)
            }
        })
    }

    /// 返回本次导航末端实际行号；无参数，短路时为 None。
    pub(crate) const fn navigation_row(&self) -> Option<i32> {
        self.navigation_row
    }

    pub(crate) async fn result_set_next(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_next(self).await
        } else {
            self.standard.next().await.map_err(DruidError::from)
        }
    }

    pub(crate) async fn result_set_close(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_close(self).await
        } else {
            self.standard.close().await.map_err(DruidError::from)
        }
    }

    /// 依注册顺序保留完整读取参数；默认委托后继续既有 value around-chain。
    /// 参数为可空已治理列；无列治理的标签读取允许在定位驱动列前短路。
    /// 对应 Java: FilterChainImpl#resultSet_getBigDecimal/getDate/getTime/getTimestamp。
    pub(crate) fn result_set_read(
        &mut self,
        column: Option<ColumnIndex>,
    ) -> Result<Value, DruidError> {
        let request = self.typed_read.ok_or_else(|| {
            DruidError::from(druid_rdbc::error::RdbcError::InvalidArgument(
                "typed result set read requires a descriptor".to_owned(),
            ))
        })?;
        if self.read_position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.read_position]);
            self.read_position += 1;
            let context = self.context;
            let label = self.typed_label;
            filter.standard_result_set_read(context, column, label, request, &mut || {
                self.result_set_read(column)
            })
        } else {
            // 未治理的标签由完整读取hook观察，末端保留原生标签重载。
            // 不能为只有下标身份的value hooks额外定位列；已治理列仍走原value链。
            if matches!(request, ResultSetRead::NString)
                || (self.typed_label.is_some() && column.is_none())
            {
                return self.typed_value(column);
            }
            // 旧 value hooks 仍观察真实驱动列，只有完整读取 hook 可以提前短路。
            let column = match column {
                Some(column) => column,
                None => self
                    .standard
                    .find_column(self.typed_label.ok_or_else(|| {
                        DruidError::from(druid_rdbc::error::RdbcError::InvalidArgument(
                            "typed result set read requires a column or label".to_owned(),
                        ))
                    })?)
                    .map_err(DruidError::from)?,
            };
            self.result_set_value(column)
        }
    }

    pub(crate) fn result_set_value(&mut self, column: ColumnIndex) -> Result<Value, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_value(self, column)
        } else if self.typed_read.is_some() {
            self.typed_value(Some(column))
        } else if self.string_read {
            self.standard.string_value(column).map_err(DruidError::from)
        } else {
            self.standard.value(column).map_err(DruidError::from)
        }
    }

    /// 仅在链末端分派原生标量；参数为可空列，标签入口保留原标签且不重新定位。
    /// 对应 Java: FilterChainImpl#resultSet_getNString 及其他强类型getter的驱动末端。
    fn typed_value(&self, column: Option<ColumnIndex>) -> Result<Value, DruidError> {
        let request = self.typed_read.ok_or_else(|| {
            DruidError::from(druid_rdbc::error::RdbcError::InvalidArgument(
                "typed result set read requires a descriptor".to_owned(),
            ))
        })?;
        let object = match self.typed_label {
            Some(label) => self.standard.read_object_by_label(label, request),
            None => self.standard.read_object(
                column.ok_or_else(|| {
                    DruidError::from(druid_rdbc::error::RdbcError::InvalidArgument(
                        "typed result set read requires a column or label".to_owned(),
                    ))
                })?,
                request,
            ),
        }
        .map_err(DruidError::from)?;
        // national字符串保留Unicode，资源句柄不得进入该标量转换。
        match object {
            RdbcObject::Scalar(value) => Ok(value),
            RdbcObject::NString(value) => Ok(Value::String(value)),
            RdbcObject::BigDecimal(value) => Ok(Value::Decimal(value)),
            RdbcObject::Date(value) => Ok(Value::Date(value)),
            RdbcObject::Time(value) => Ok(Value::Time(value)),
            RdbcObject::Timestamp(value) => Ok(Value::Timestamp(value)),
            other => Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                format!("ResultSet typed scalar getter returned {other}"),
            ))),
        }
    }

    /// 依注册顺序转发下标更新；参数为列和描述符，返回末端或短路结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(int, ...)。
    pub(crate) fn result_set_update(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_update(self, column, update)
        } else {
            self.standard
                .update_value(column, update)
                .map_err(DruidError::from)
        }
    }

    /// 保留原列名逐层转发更新；参数为列名和描述符，返回末端或短路结果。
    /// 对应 Java: FilterChainImpl#resultSet_updateXxx(String, ...)。
    pub(crate) fn result_set_update_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_update_by_label(self, label, update)
        } else {
            self.standard
                .update_value_by_label(label, update)
                .map_err(DruidError::from)
        }
    }

    pub(crate) fn result_set_meta_data(
        &mut self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_meta_data(self)
        } else {
            self.standard.meta_data().map_err(DruidError::from)
        }
    }

    pub(crate) fn result_set_was_null(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_was_null(self)
        } else {
            self.standard.was_null().map_err(DruidError::from)
        }
    }

    pub(crate) fn context(&self) -> &'a ResultSetFilterContext {
        self.context
    }

    pub(crate) fn raw_value(&self, column_index: usize) -> Result<Value, DruidError> {
        let column = ColumnIndex::new(column_index).map_err(DruidError::from)?;
        self.standard.value(column).map_err(DruidError::from)
    }
}
