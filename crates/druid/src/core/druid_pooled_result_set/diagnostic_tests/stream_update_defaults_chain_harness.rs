use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use std::sync::Arc;
/// 原二十二个流更新入口桥接到真实标准链，长度与共享资源原样交接。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct StreamUpdateDefaultsChainHarness {
    chain: FilterChain,
}
impl StreamUpdateDefaultsChainHarness {
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
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream_by_label_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_ascii_stream_by_label_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::AsciiStream {
                stream: value,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream_by_label_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_binary_stream_by_label_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::BinaryStream {
                stream: value,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcReader>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcReader>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcReader>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream_by_label_with_int_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcReader>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Int(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_character_stream_by_label_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::CharacterStream {
                reader: value,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_n_character_stream(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcReader>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::NCharacterStream {
                reader: value,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_n_character_stream_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        position: usize,
        value: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update(
            raw,
            context,
            ColumnIndex::new(position).map_err(DruidError::from)?,
            ResultSetUpdate::NCharacterStream {
                reader: value,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_n_character_stream_by_label(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcReader>,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::NCharacterStream {
                reader: value,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn result_set_update_n_character_stream_by_label_with_length(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
        value: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_update_by_label(
            raw,
            context,
            label,
            ResultSetUpdate::NCharacterStream {
                reader: value,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
}
