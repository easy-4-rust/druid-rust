use crate::SqliteQueryColumn;
use std::sync::{Arc, Mutex};

/// 仅属于一个真实SQLite连接的单次执行列元数据交接槽。
/// 对应 Java: java.sql.Statement#getResultSet 与ResultSetMetaData的同次结果所有权。
/// 不是数据库执行接口；官方Toasty Connection::exec仍执行唯一一次原SQL。
#[derive(Clone, Debug, Default)]
pub struct SqliteQueryMetadata {
    columns: Arc<Mutex<Option<Vec<SqliteQueryColumn>>>>,
}

impl SqliteQueryMetadata {
    /// 在新的原生prepare前清除上一结果；无参数和返回值，防止失败后泄漏旧列。
    pub fn clear(&self) {
        *self
            .columns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// 保存本次原生prepare提供的全部列；空列表表示实际无结果列。
    /// 参数为原生列快照，无返回值；调用方必须处于同一连接执行锁范围。
    pub(crate) fn replace_columns(&self, columns: Vec<SqliteQueryColumn>) {
        *self
            .columns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(columns);
    }

    /// 在本次查询成功后取走原生列快照；返回None表示驱动没有提供快照。
    /// 移出后清空槽位，不让后续执行复用先前结果的标签。
    pub fn take_columns(&self) -> Option<Vec<SqliteQueryColumn>> {
        self.columns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}
