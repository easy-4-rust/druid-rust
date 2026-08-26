//! Canonical standard PreparedStatement 的 Druid 私有缓存。

use super::prepared_statement_key::PreparedStatementKey;
use druid_rdbc::statement::PreparedStatement;
use std::collections::{HashMap, VecDeque};

/// 单 raw connection 的 access-order standard PreparedStatement cache。
pub(crate) struct StandardPreparedStatementCache {
    entries: HashMap<PreparedStatementKey, Box<dyn PreparedStatement>>,
    access_order: VecDeque<PreparedStatementKey>,
    pending_close: Vec<Box<dyn PreparedStatement>>,
    max_size: usize,
}

impl StandardPreparedStatementCache {
    pub(crate) fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_size.max(1)),
            access_order: VecDeque::with_capacity(max_size.max(1)),
            pending_close: Vec::new(),
            max_size,
        }
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        self.max_size > 0
    }

    pub(crate) fn take(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Option<Box<dyn PreparedStatement>> {
        let statement = self.entries.remove(key);
        self.remove_order(key);
        statement
    }

    pub(crate) fn put(
        &mut self,
        key: PreparedStatementKey,
        statement: Box<dyn PreparedStatement>,
    ) -> Vec<Box<dyn PreparedStatement>> {
        let mut removed = Vec::new();
        if let Some(previous) = self.entries.insert(key.clone(), statement) {
            removed.push(previous);
        }
        self.remove_order(&key);
        self.access_order.push_back(key);
        while self.entries.len() > self.max_size {
            let Some(eldest) = self.access_order.pop_front() else {
                break;
            };
            if let Some(statement) = self.entries.remove(&eldest) {
                removed.push(statement);
            }
        }
        removed
    }

    pub(crate) fn clear(&mut self) -> Vec<Box<dyn PreparedStatement>> {
        self.access_order.clear();
        let mut statements = self
            .entries
            .drain()
            .map(|(_, statement)| statement)
            .collect::<Vec<_>>();
        statements.append(&mut self.pending_close);
        statements
    }

    pub(crate) fn retain_failed_close(&mut self, mut statements: Vec<Box<dyn PreparedStatement>>) {
        self.pending_close.append(&mut statements);
    }

    fn remove_order(&mut self, key: &PreparedStatementKey) {
        if let Some(index) = self
            .access_order
            .iter()
            .position(|candidate| candidate == key)
        {
            self.access_order.remove(index);
        }
    }
}
