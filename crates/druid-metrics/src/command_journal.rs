use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

type CommandResult = Result<(), String>;
type CommandRecord = (Vec<u8>, Option<CommandResult>);

/// 客户端有界命令执行日志。
///
/// Rust-only 可靠传输对象；先登记 canonical payload，再执行副作用并持久化结果。
/// 同进程重连只重放结果，不再次执行副作用。
#[derive(Debug)]
pub(crate) struct CommandJournal {
    capacity: usize,
    state: Mutex<(HashMap<String, CommandRecord>, VecDeque<String>)>,
}

impl CommandJournal {
    /// 创建指定容量的命令日志。
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            state: Mutex::new((HashMap::new(), VecDeque::new())),
        }
    }

    /// 登记命令；返回 `(是否执行, 已有稳定结果)`。
    pub(crate) fn begin(
        &self,
        command_id: &str,
        canonical_payload: Vec<u8>,
    ) -> Result<(bool, Option<CommandResult>), String> {
        if command_id.trim().is_empty() {
            return Err("empty command ID".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((stored_payload, result)) = state.0.get(command_id) {
            if stored_payload != &canonical_payload {
                return Err("command ID has a conflicting canonical payload".to_owned());
            }
            return Ok((false, result.clone()));
        }
        while state.0.len() >= self.capacity {
            let completed = state
                .1
                .iter()
                .position(|id| state.0.get(id).is_some_and(|record| record.1.is_some()));
            let Some(index) = completed else {
                return Err(format!("command journal full: capacity {}", self.capacity));
            };
            let evicted = state
                .1
                .remove(index)
                .expect("completed command index was validated");
            state.0.remove(&evicted);
        }
        state
            .0
            .insert(command_id.to_owned(), (canonical_payload, None));
        state.1.push_back(command_id.to_owned());
        Ok((true, None))
    }

    /// 在副作用返回后保存稳定成功或失败结果。
    pub(crate) fn complete(&self, command_id: &str, result: CommandResult) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(record) = state.0.get_mut(command_id) {
            record.1 = Some(result);
        }
    }
}
