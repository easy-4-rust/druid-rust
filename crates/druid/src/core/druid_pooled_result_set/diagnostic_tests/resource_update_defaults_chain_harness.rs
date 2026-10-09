use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::RdbcRowId;
use std::sync::Arc;
/// 原十四个资源更新入口桥接到实际标准链，不改变资源或参数。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct ResourceUpdateDefaultsChainHarness {
    chain: FilterChain,
}
impl ResourceUpdateDefaultsChainHarness {
    /// 创建链桥接；无参数，返回空链。
    pub(super) fn new() -> Self {
        Self {
            chain: FilterChain::new(),
        }
    }
    /// 注册原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.chain.add_result_set(filter);
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_reference(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcRef>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::Ref(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_reference_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcRef>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::Ref(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_blob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcBlob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::Blob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_blob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcBlob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::Blob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_clob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcClob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::Clob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_clob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcClob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::Clob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_array(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcArray>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::Array(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_array_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcArray>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::Array(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_row_id(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcRowId>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::RowId(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_row_id_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcRowId>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::RowId(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_n_clob(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcNClob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::NClob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_n_clob_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcNClob>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::NClob(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_sql_xml(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::SqlXml(value),
        )
    }
    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn result_set_update_sql_xml_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::SqlXml(value),
        )
    }
}
