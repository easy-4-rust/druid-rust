use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use druid_rdbc::types::{RdbcCharacterLength, RdbcStreamLength};
use std::sync::{Arc, Mutex};

/// 原LOB更新Filter的完整短路行为。对应 Java: Filter#resultSet_updateBlob/updateClob/updateNClob。
pub(super) struct LobStreamUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}
impl LobStreamUpdateShortCircuitFilter {
    /// 保存真实Filter调用日志，返回可注册的Filter。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self { calls }
    }

    /// 保留原更新重载并消费指定首元素，返回是否为本Filter的LOB家族。
    fn handle(
        &self,
        target: &str,
        label: bool,
        update: &ResultSetUpdate,
    ) -> Result<bool, DruidError> {
        let (family, present, length) = match update {
            ResultSetUpdate::BlobStream { stream, length } => {
                if !label && matches!(length, RdbcStreamLength::Unspecified) {
                    if let Some(stream) = stream {
                        let mut byte = [0u8; 1];
                        assert_eq!(stream.read(&mut byte).map_err(DruidError::from)?, 1);
                    }
                }
                let length = match length {
                    RdbcStreamLength::Unspecified => None,
                    RdbcStreamLength::Int(length) => Some(i64::from(*length)),
                    RdbcStreamLength::Long(length) => Some(*length),
                };
                ("blob_stream", stream.is_some(), length)
            }
            ResultSetUpdate::ClobReader { reader, length }
            | ResultSetUpdate::NClobReader { reader, length } => {
                if !label && matches!(length, RdbcCharacterLength::Unspecified) {
                    if let Some(reader) = reader {
                        let mut unit = [0u16; 1];
                        assert_eq!(reader.read_utf16(&mut unit).map_err(DruidError::from)?, 1);
                    }
                }
                let length = match length {
                    RdbcCharacterLength::Unspecified => None,
                    RdbcCharacterLength::Int(length) => Some(i64::from(*length)),
                    RdbcCharacterLength::Long(length) => Some(*length),
                };
                let family = if matches!(update, ResultSetUpdate::ClobReader { .. }) {
                    "clob_reader"
                } else {
                    "n_clob_reader"
                };
                (family, reader.is_some(), length)
            }
            _ => return Ok(false),
        };
        let label_suffix = if label { "_by_label" } else { "" };
        let length_suffix = if length.is_some() { "_with_length" } else { "" };
        let presence = if present { "some" } else { "none" };
        let suffix = length.map_or_else(String::new, |length| format!(":{length}"));
        self.calls.lock().unwrap().push(format!(
            "result_set_update_{family}{label_suffix}{length_suffix}:{target}:{presence}{suffix}"
        ));
        if label && target == "fail" {
            return Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered LOB stream update failure".to_owned(),
            )));
        }
        Ok(true)
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for LobStreamUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if self.handle(&column.get().to_string(), false, &update)? {
            Ok(())
        } else {
            chain.result_set_update(column, update)
        }
    }
    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if self.handle(label, true, &update)? {
            Ok(())
        } else {
            chain.result_set_update_by_label(label, update)
        }
    }
}
