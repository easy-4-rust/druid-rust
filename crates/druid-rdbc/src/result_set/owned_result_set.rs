use super::ResultSet;

/// 保证 owning Stream 在任意丢弃或取消位置交接唯一标准游标。
/// 对应 Java: java.sql.ResultSet#close 的 Rust 所有权保护，不定义第二套游标接口。
pub(super) struct OwnedResultSet<T: ResultSet + ?Sized> {
    cursor: Option<Box<T>>,
}

impl<T: ResultSet + ?Sized> OwnedResultSet<T> {
    /// 接收唯一标准游标；参数为拥有型句柄，返回供 Stream 持有的释放保护。
    pub(super) fn new(cursor: Box<T>) -> Self {
        Self {
            cursor: Some(cursor),
        }
    }

    /// 借用当前标准游标；无参数，返回只读引用，不转移所有权。
    pub(super) fn as_ref(&self) -> &T {
        self.cursor.as_deref().expect("live stream owns its cursor")
    }

    /// 可变借用当前标准游标；无参数，返回供推进或关闭使用的引用。
    pub(super) fn as_mut(&mut self) -> &mut T {
        self.cursor
            .as_deref_mut()
            .expect("live stream owns its cursor")
    }
}

impl<T: ResultSet + ?Sized> Drop for OwnedResultSet<T> {
    fn drop(&mut self) {
        if let Some(cursor) = self.cursor.take() {
            cursor.release_owned();
        }
    }
}
