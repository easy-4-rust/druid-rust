use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcReader, RdbcRef, RdbcSqlXml,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::types::{RdbcObject, RdbcRowId, RdbcUrl};
use std::sync::Arc;
/// 原重载入口桥接到实际标准过滤器链，资源按真实类型转换。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct ResourceDefaultsChainHarness {
    chain: FilterChain,
}
impl ResourceDefaultsChainHarness {
    /// 创建默认链桥接；无参数，返回空链。
    pub(super) fn new() -> Self {
        Self {
            chain: FilterChain::new(),
        }
    }
    /// 注册过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.chain.add_result_set(filter);
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_ref(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcRef>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::Ref,
            None,
        )?;
        match value {
            RdbcObject::Ref(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_ref_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcRef>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::Ref,
            Some(label),
        )?;
        match value {
            RdbcObject::Ref(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_blob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcBlob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::Blob,
            None,
        )?;
        match value {
            RdbcObject::Blob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_blob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcBlob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::Blob,
            Some(label),
        )?;
        match value {
            RdbcObject::Blob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_clob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcClob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::Clob,
            None,
        )?;
        match value {
            RdbcObject::Clob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_clob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcClob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::Clob,
            Some(label),
        )?;
        match value {
            RdbcObject::Clob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_array(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcArray>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::Array,
            None,
        )?;
        match value {
            RdbcObject::Array(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_array_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcArray>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::Array,
            Some(label),
        )?;
        match value {
            RdbcObject::Array(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_url(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcUrl>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::Url,
            None,
        )?;
        match value {
            RdbcObject::Url(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_url_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcUrl>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::Url,
            Some(label),
        )?;
        match value {
            RdbcObject::Url(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_row_id(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcRowId>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::RowId,
            None,
        )?;
        match value {
            RdbcObject::RowId(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_row_id_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcRowId>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::RowId,
            Some(label),
        )?;
        match value {
            RdbcObject::RowId(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_n_clob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcNClob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::NClob,
            None,
        )?;
        match value {
            RdbcObject::NClob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_n_clob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcNClob>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::NClob,
            Some(label),
        )?;
        match value {
            RdbcObject::NClob(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_sql_xml(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcSqlXml>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            &ResultSetRead::SqlXml,
            None,
        )?;
        match value {
            RdbcObject::SqlXml(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn result_set_get_sql_xml_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcSqlXml>, DruidError> {
        let value = self.chain.standard_result_set_resource(
            raw,
            context,
            None,
            &ResultSetRead::SqlXml,
            Some(label),
        )?;
        match value {
            RdbcObject::SqlXml(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_ascii_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            None,
            ResultSetStreamKind::Ascii,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_ascii_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            None,
            Some(label),
            ResultSetStreamKind::Ascii,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_unicode_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            None,
            ResultSetStreamKind::Unicode,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_unicode_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            None,
            Some(label),
            ResultSetStreamKind::Unicode,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_binary_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            None,
            ResultSetStreamKind::Binary,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_binary_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.chain.standard_result_set_input_stream(
            raw,
            context,
            None,
            Some(label),
            ResultSetStreamKind::Binary,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_character_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcReader>, DruidError> {
        self.chain.standard_result_set_reader(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            None,
            false,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_character_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcReader>, DruidError> {
        self.chain
            .standard_result_set_reader(raw, context, None, Some(label), false)
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_n_character_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
    ) -> Result<Option<RdbcReader>, DruidError> {
        self.chain.standard_result_set_reader(
            raw,
            context,
            Some(ColumnIndex::new(position).map_err(DruidError::from)?),
            None,
            true,
        )
    }
    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn result_set_get_n_character_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<RdbcReader>, DruidError> {
        self.chain
            .standard_result_set_reader(raw, context, None, Some(label), true)
    }
}
