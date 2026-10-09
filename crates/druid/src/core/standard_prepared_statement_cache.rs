//! Canonical standard PreparedStatement 的 Druid 私有缓存。

use super::cached_prepared_statement::CachedPreparedStatement;
use super::prepared_statement_key::PreparedStatementKey;
use druid_rdbc::statement::{CallableStatement, PreparedStatement};
use std::collections::{HashMap, VecDeque};

type ClearedPreparedStatements = (Vec<CachedPreparedStatement>, Vec<CachedPreparedStatement>);

/// 单 raw connection 的 access-order standard PreparedStatement cache。
pub(crate) struct StandardPreparedStatementCache {
    entries: HashMap<PreparedStatementKey, CachedPreparedStatement>,
    access_order: VecDeque<PreparedStatementKey>,
    pending_close: Vec<CachedPreparedStatement>,
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
        if !matches!(
            self.entries.get(key),
            Some(CachedPreparedStatement::Prepared(_))
        ) {
            return None;
        }
        let statement = self.entries.remove(key);
        self.remove_order(key);
        match statement {
            Some(CachedPreparedStatement::Prepared(statement)) => Some(statement),
            _ => None,
        }
    }

    /// 只借出 Callable 条目，禁止擦除类型后把普通 Prepared 当作 Callable。
    pub(crate) fn take_callable(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Option<Box<dyn CallableStatement>> {
        if !matches!(
            self.entries.get(key),
            Some(CachedPreparedStatement::Callable(_))
        ) {
            return None;
        }
        let statement = self.entries.remove(key);
        self.remove_order(key);
        match statement {
            Some(CachedPreparedStatement::Callable(statement)) => Some(statement),
            _ => None,
        }
    }

    pub(crate) fn put(
        &mut self,
        key: PreparedStatementKey,
        statement: Box<dyn PreparedStatement>,
    ) -> Vec<CachedPreparedStatement> {
        self.put_entry(key, statement.into())
    }

    /// 把 Callable 放回同一 LRU，容量与 Prepared 共用。
    pub(crate) fn put_callable(
        &mut self,
        key: PreparedStatementKey,
        statement: Box<dyn CallableStatement>,
    ) -> Vec<CachedPreparedStatement> {
        self.put_entry(key, statement.into())
    }

    fn put_entry(
        &mut self,
        key: PreparedStatementKey,
        statement: CachedPreparedStatement,
    ) -> Vec<CachedPreparedStatement> {
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

    pub(crate) fn clear(&mut self) -> ClearedPreparedStatements {
        self.access_order.clear();
        let statements = self
            .entries
            .drain()
            .map(|(_, statement)| statement)
            .collect::<Vec<_>>();
        let pending_close = std::mem::take(&mut self.pending_close);
        (statements, pending_close)
    }

    pub(crate) fn retain_failed_close<T: Into<CachedPreparedStatement>>(
        &mut self,
        statements: Vec<T>,
    ) {
        self.pending_close
            .extend(statements.into_iter().map(Into::into));
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
