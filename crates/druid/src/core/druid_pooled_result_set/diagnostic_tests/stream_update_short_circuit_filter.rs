use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use druid_rdbc::types::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use std::sync::{Arc, Mutex};

/// 完整保留22个流更新重载的原事件、长度、共享流消费和精确错误。
/// 对应 Java: Filter#resultSet_updateAsciiStream/updateBinaryStream/updateCharacterStream/updateNCharacterStream。
pub(super) struct StreamUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
impl StreamUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }
    fn record_stream(
        &self,
        name: &str,
        target: &str,
        by_label: bool,
        value: Option<RdbcInputStream>,
        length: RdbcStreamLength,
    ) -> Result<(), DruidError> {
        let presence = if value.is_some() { "some" } else { "none" };
        let label_suffix = if by_label { "_by_label" } else { "" };
        let (overload, tail) = match length {
            RdbcStreamLength::Unspecified => {
                // 只在原无长度下标入口消费一个字节；clone必须共享调用者读位置。
                if !by_label {
                    if let Some(value) = value {
                        let mut byte = [0u8; 1];
                        assert_eq!(value.read(&mut byte)?, 1);
                    }
                }
                ("", String::new())
            }
            RdbcStreamLength::Int(length) => ("_with_int_length", format!(":{length}")),
            RdbcStreamLength::Long(length) => ("_with_length", format!(":{length}")),
        };
        self.record(format!(
            "result_set_update_{name}{label_suffix}{overload}:{target}:{presence}{tail}"
        ));
        Ok(())
    }
    fn record_reader(
        &self,
        name: &str,
        target: &str,
        by_label: bool,
        value: Option<RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), DruidError> {
        let presence = if value.is_some() { "some" } else { "none" };
        let label_suffix = if by_label { "_by_label" } else { "" };
        let long_overload = matches!(length, RdbcCharacterLength::Long(_));
        let (overload, tail) = match length {
            RdbcCharacterLength::Unspecified => {
                // UTF-16单位消费与原Reader合同相同，带长度入口不得预读。
                if !by_label {
                    if let Some(value) = value {
                        let mut unit = [0u16; 1];
                        assert_eq!(value.read_utf16(&mut unit)?, 1);
                    }
                }
                ("", String::new())
            }
            RdbcCharacterLength::Int(length) => ("_with_int_length", format!(":{length}")),
            RdbcCharacterLength::Long(length) => ("_with_length", format!(":{length}")),
        };
        self.record(format!(
            "result_set_update_{name}{label_suffix}{overload}:{target}:{presence}{tail}"
        ));
        if name == "n_character_stream" && by_label && long_overload && target == "fail" {
            Err(DruidError::DriverError(
                "filtered stream update failure".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for StreamUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::AsciiStream { stream, length } => self.record_stream(
                "ascii_stream",
                &column.get().to_string(),
                false,
                stream,
                length,
            ),
            ResultSetUpdate::BinaryStream { stream, length } => self.record_stream(
                "binary_stream",
                &column.get().to_string(),
                false,
                stream,
                length,
            ),
            ResultSetUpdate::CharacterStream { reader, length } => self.record_reader(
                "character_stream",
                &column.get().to_string(),
                false,
                reader,
                length,
            ),
            ResultSetUpdate::NCharacterStream { reader, length } => self.record_reader(
                "n_character_stream",
                &column.get().to_string(),
                false,
                reader,
                length,
            ),
            other => chain.result_set_update(column, other),
        }
    }
    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::AsciiStream { stream, length } => {
                self.record_stream("ascii_stream", label, true, stream, length)
            }
            ResultSetUpdate::BinaryStream { stream, length } => {
                self.record_stream("binary_stream", label, true, stream, length)
            }
            ResultSetUpdate::CharacterStream { reader, length } => {
                self.record_reader("character_stream", label, true, reader, length)
            }
            ResultSetUpdate::NCharacterStream { reader, length } => {
                self.record_reader("n_character_stream", label, true, reader, length)
            }
            other => chain.result_set_update_by_label(label, other),
        }
    }
}
