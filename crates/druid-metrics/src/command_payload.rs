use crate::protocol::command;

/// 把远程管理命令规范化为不含 header、时间或凭据的稳定 payload 字节。
///
/// ResetStats 的 datasource ID 按数值排序并去重，因此相同目标集合具有相同
/// canonical payload；RequestFullSnapshot 使用固定单字节标识。
pub fn canonical_command_payload(
    payload: &Option<command::Payload>,
) -> Result<Vec<u8>, &'static str> {
    match payload {
        Some(command::Payload::ResetStats(reset)) => {
            let mut ids = reset.target_datasource_ids.clone();
            ids.sort_unstable();
            ids.dedup();
            let mut canonical = Vec::with_capacity(1 + ids.len().saturating_mul(8));
            canonical.push(1);
            for datasource_id in ids {
                canonical.extend_from_slice(&datasource_id.to_be_bytes());
            }
            Ok(canonical)
        }
        Some(command::Payload::RequestFullSnapshot(_)) => Ok(vec![2]),
        None => Err("command payload is empty"),
    }
}
