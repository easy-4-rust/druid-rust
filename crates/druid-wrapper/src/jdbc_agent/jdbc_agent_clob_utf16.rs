use base64::Engine;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::RdbcString;
use serde_json::{json, Value as JsonValue};

/// 在 JDBC Agent 边界保留 Java String 的原始 UTF-16 单元，包括孤立代理项。
/// 对应 Java: AgentServer#encodeUtf16 与 AgentServer#decodeUtf16。
pub(super) struct JdbcAgentClobUtf16;

impl JdbcAgentClobUtf16 {
    /// 将标准字符串编码成协议的 big-endian UTF-16 Base64 字段。
    pub(super) fn encode(value: &RdbcString) -> JsonValue {
        let bytes: Vec<u8> = value
            .as_utf16()
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect();
        json!(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// 严格解码响应字段；无字段、非法 Base64 或奇数字节数都视为协议错误。
    pub(super) fn decode(response: &JsonValue) -> Result<RdbcString, RdbcError> {
        let encoded = response
            .get("utf16")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| Self::protocol_error("Clob response has no utf16 field"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| {
                Self::protocol_error(format!("invalid Clob UTF-16 Base64: {error}"))
            })?;
        if bytes.len() % 2 != 0 {
            return Err(Self::protocol_error(
                "Clob UTF-16 payload has an odd byte count",
            ));
        }
        Ok(RdbcString::from_utf16(
            bytes
                .chunks_exact(2)
                .map(|unit| u16::from_be_bytes([unit[0], unit[1]]))
                .collect::<Vec<_>>(),
        ))
    }

    fn protocol_error(message: impl Into<String>) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent protocol error: {}", message.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::JdbcAgentClobUtf16;
    use druid_rdbc::types::RdbcString;
    use serde_json::json;

    #[test]
    fn preserves_supplementary_and_isolated_utf16_units() {
        let source = RdbcString::from_utf16(vec![0x0041, 0xd83d, 0xde00, 0xd800, 0x0042]);
        let wire = json!({"utf16": JdbcAgentClobUtf16::encode(&source)});
        assert_eq!(JdbcAgentClobUtf16::decode(&wire).expect("decode"), source);
    }

    #[test]
    fn rejects_corrupt_or_missing_wire_values() {
        for response in [
            json!({}),
            json!({"utf16": null}),
            json!({"utf16": "AA=="}),
            json!({"utf16": "!"}),
        ] {
            assert!(JdbcAgentClobUtf16::decode(&response).is_err());
        }
    }
}
