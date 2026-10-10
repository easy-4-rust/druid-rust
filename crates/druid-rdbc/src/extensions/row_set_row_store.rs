use super::row_set_stored_row::RowSetStoredRow;
use super::RowSetRowChange;
use crate::error::{RdbcError, SqlException};
use crate::result_set::ColumnIndex;
use crate::types::RdbcObject;

/// RowSet 原始行、当前行和变更的 rich-object 状态；不运行数据库或发送事件。
/// 对应 Java: javax.sql.RowSetInternal#getOriginal/getOriginalRow 与 RowSetWriter#writeData。
/// 本 Rust 扩展返回行数据，完整 RowSet 仍须装配标准 ResultSet 和游标、事件及 provider 调用。
/// 克隆对象引用保留资源身份，不对外部可变 LOB/Reader 内容做深拷贝或主动 close/free。
#[derive(Clone)]
pub struct RowSetRowStore {
    column_count: usize,
    rows: Vec<RowSetStoredRow>,
}

impl std::fmt::Debug for RowSetRowStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowSetRowStore")
            .field("column_count", &self.column_count)
            .field("stored_row_count", &self.rows.len())
            .finish_non_exhaustive()
    }
}

impl RowSetRowStore {
    /// 返回当前可见的 1-based 槽位；内部游标不把删除行暴露成正常行。
    pub(crate) fn visible_slots(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| (!row.deleted).then_some(index + 1))
            .collect()
    }
    /// 返回当前可见值快照，包含 overlay；保留 rich object 引用，不消费资源。
    pub(crate) fn visible_rows(&self) -> Vec<Vec<RdbcObject>> {
        self.rows
            .iter()
            .filter(|row| !row.deleted)
            .map(|row| {
                row.values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| row.pending.get(&(index + 1)).unwrap_or(value).clone())
                    .collect()
            })
            .collect()
    }
    /// 返回指定有效槽位的更新与插入标志，不改变原始基线。
    pub(crate) fn row_flags(&self, index: usize) -> Result<(bool, bool), RdbcError> {
        let row = self.row(index)?;
        Ok((row.updated, row.original.is_none()))
    }
    /// 以 column_count 创建空状态；不按列数预分配，返回尚无原始行的存储。
    pub fn new(column_count: usize) -> Self {
        Self {
            column_count,
            rows: Vec::new(),
        }
    }

    /// 返回存储槽位数，无参数；包含尚未接受写回的删除标记行。
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// 返回是否没有任何槽位，无参数；逻辑可见行由完整 RowSet 游标另行筛选。
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 使用参数 rows 替换完整读取基线，返回成功或宽度 SQL 异常。
    /// 先校验全部行再修改状态；每行成为原始行，旧状态仅释放自身引用，不主动关闭外部资源。
    pub fn replace_rows(&mut self, rows: Vec<Vec<RdbcObject>>) -> Result<(), RdbcError> {
        if rows.iter().any(|row| row.len() != self.column_count) {
            return Err(Self::row_error("RowSet row width does not match metadata"));
        }
        self.rows = rows.into_iter().map(RowSetStoredRow::original).collect();
        Ok(())
    }

    /// 返回所有原始行的非消费快照，无参数；排除新增行，包含标记删除的原有行。
    /// 对应 Java: RowSetInternal#getOriginal 的数据部分；完整标准返回须使用 before-first ResultSet。
    pub fn original_rows(&self) -> Vec<Vec<RdbcObject>> {
        self.rows
            .iter()
            .filter_map(|row| row.original.clone())
            .collect()
    }

    /// 返回参数 row_index 对应的单行原始快照；新增行返回空结果，没有当前行或非法槽位返回 SQL 异常。
    /// None 必须由完整游标用于 insert/before/after 状态，Some 为 1-based 内部槽位。
    /// 对应 Java: RowSetInternal#getOriginalRow 的数据和错误部分。
    pub fn original_row(
        &self,
        row_index: Option<usize>,
    ) -> Result<Vec<Vec<RdbcObject>>, RdbcError> {
        let row_index = row_index.ok_or_else(|| Self::row_error("RowSet has no current row"))?;
        Ok(self.row(row_index)?.original.clone().into_iter().collect())
    }

    /// 返回 row_index 与 column_index 的当前值；包括尚未 update_row 的 overlay，不消费资源。
    /// 非法行/列或删除行返回 SQL 异常；两个下标均为 1-based。
    pub fn value(
        &self,
        row_index: usize,
        column_index: ColumnIndex,
    ) -> Result<RdbcObject, RdbcError> {
        self.check_column(column_index)?;
        let row = self.live_row(row_index)?;
        Ok(row
            .pending
            .get(&column_index.get())
            .unwrap_or(&row.values[column_index.get() - 1])
            .clone())
    }

    /// 将 value 暂存到指定 row_index/column_index，返回成功或 SQL 异常。
    /// 两下标均为 1-based；同一列最后写入生效，原始值和已提交当前值不改变。
    pub fn stage_update(
        &mut self,
        row_index: usize,
        column_index: ColumnIndex,
        value: RdbcObject,
    ) -> Result<(), RdbcError> {
        self.check_column(column_index)?;
        self.live_row_mut(row_index)?
            .pending
            .insert(column_index.get(), value);
        Ok(())
    }

    /// 提交参数 row_index 的待更新列到当前行，返回成功或无效/删除行 SQL 异常。
    /// 不接受新的原始基线；无待更新时保持原状态，对应 Java: ResultSet#updateRow 的本地状态部分。
    pub fn update_row(&mut self, row_index: usize) -> Result<(), RdbcError> {
        let row = self.live_row_mut(row_index)?;
        if !row.pending.is_empty() {
            for (column, value) in std::mem::take(&mut row.pending) {
                row.values[column - 1] = value;
            }
            row.updated = row.original.is_some();
        }
        Ok(())
    }

    /// 取消参数 row_index 的未提交列更新，返回成功或无效/删除行 SQL 异常。
    /// 不回滚此前 update_row 已提交的值，不影响其他行。对应 Java: ResultSet#cancelRowUpdates 的本地状态部分。
    pub fn cancel_row_updates(&mut self, row_index: usize) -> Result<(), RdbcError> {
        self.live_row_mut(row_index)?.pending.clear();
        Ok(())
    }

    /// 插入参数 values 的完整行，返回新的 1-based 槽位或宽度 SQL 异常。
    /// 新行没有原始值；完整 RowSet 须先处理 insert-row 游标、默认列值及更新能力。
    pub fn insert_row(&mut self, values: Vec<RdbcObject>) -> Result<usize, RdbcError> {
        if values.len() != self.column_count {
            return Err(Self::row_error(
                "RowSet inserted row width does not match metadata",
            ));
        }
        self.rows.push(RowSetStoredRow::inserted(values));
        Ok(self.rows.len())
    }

    /// 标记参数 row_index 为删除并取消该行未提交更新，返回成功或无效/已删除 SQL 异常。
    /// 保留原始值供 writer 检查冲突；新增后删除不产生数据源写回。
    pub fn delete_row(&mut self, row_index: usize) -> Result<(), RdbcError> {
        let row = self.live_row_mut(row_index)?;
        row.deleted = true;
        row.pending.clear();
        Ok(())
    }

    /// 查询参数 row_index 的删除标记，返回 bool 或无效槽位 SQL 异常，不改动游标。
    pub fn is_deleted(&self, row_index: usize) -> Result<bool, RdbcError> {
        Ok(self.row(row_index)?.deleted)
    }

    /// 返回所有已提交的本地变化快照，无参数；有未提交列更新时返回 SQL 异常。
    /// 此操作不接受新基线、不消费资源，也不运行 provider；writer false/Err/取消时必须保留原状态。
    pub fn changes(&self) -> Result<Vec<RowSetRowChange>, RdbcError> {
        self.ensure_no_pending()?;
        let mut changes = Vec::new();
        for (offset, row) in self.rows.iter().enumerate() {
            let row_index = offset + 1;
            match (&row.original, row.deleted, row.updated) {
                (None, false, _) => changes.push(RowSetRowChange::Inserted {
                    row_index,
                    values: row.values.clone(),
                }),
                (Some(original), true, _) => changes.push(RowSetRowChange::Deleted {
                    row_index,
                    original: original.clone(),
                }),
                (Some(original), false, true) => changes.push(RowSetRowChange::Updated {
                    row_index,
                    original: original.clone(),
                    values: row.values.clone(),
                }),
                _ => {}
            }
        }
        Ok(changes)
    }

    /// 在调用方确认 writer 实际成功后接受所有变化，无参数；有 pending 时拒绝且不修改状态。
    /// 删除行被移除，剩余当前值成为新原始基线并重建槽位；不执行数据库事务或关闭外部资源。
    pub fn accept_changes(&mut self) -> Result<(), RdbcError> {
        self.ensure_no_pending()?;
        self.rows.retain(|row| !row.deleted);
        for row in &mut self.rows {
            row.original = Some(row.values.clone());
            row.updated = false;
        }
        Ok(())
    }

    /// 恢复全部原始行，无参数、无返回值；取消本地更新和删除，丢弃新增并重建槽位。
    /// 此 Rust 扩展只恢复本地状态，不承诺回滚已发生的数据源写入。
    pub fn restore_originals(&mut self) {
        self.rows = self
            .rows
            .drain(..)
            .filter_map(|row| row.original.map(RowSetStoredRow::original))
            .collect();
    }

    fn ensure_no_pending(&self) -> Result<(), RdbcError> {
        if self.rows.iter().any(|row| !row.pending.is_empty()) {
            return Err(Self::row_error("RowSet has uncommitted column updates"));
        }
        Ok(())
    }

    fn check_column(&self, column_index: ColumnIndex) -> Result<(), RdbcError> {
        if column_index.get() > self.column_count {
            return Err(Self::row_error("RowSet column index is out of range"));
        }
        Ok(())
    }

    fn row(&self, row_index: usize) -> Result<&RowSetStoredRow, RdbcError> {
        row_index
            .checked_sub(1)
            .and_then(|index| self.rows.get(index))
            .ok_or_else(|| Self::row_error("RowSet row index is out of range"))
    }

    fn live_row(&self, row_index: usize) -> Result<&RowSetStoredRow, RdbcError> {
        let row = self.row(row_index)?;
        if row.deleted {
            return Err(Self::row_error("RowSet current row is deleted"));
        }
        Ok(row)
    }

    fn live_row_mut(&mut self, row_index: usize) -> Result<&mut RowSetStoredRow, RdbcError> {
        let row = row_index
            .checked_sub(1)
            .and_then(|index| self.rows.get_mut(index))
            .ok_or_else(|| Self::row_error("RowSet row index is out of range"))?;
        if row.deleted {
            return Err(Self::row_error("RowSet current row is deleted"));
        }
        Ok(row)
    }

    fn row_error(message: &'static str) -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(
            0,
            Some("HY010".into()),
            Some(message.into()),
        )))
    }
}
