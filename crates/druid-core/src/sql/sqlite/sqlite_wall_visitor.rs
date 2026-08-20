//! 对应 Java：`com.alibaba.druid.wall.spi.SQLiteWallVisitor`。

use crate::sql::{
    DbType, WallProvider, WallUpdateCheckItem, WallViolation, WallVisitor, WallVisitorBase,
};
use sqlparser::ast::Statement;

/// `SQLite` Wall 方言 Visitor。
pub struct SQLiteWallVisitor<'a> {
    base: WallVisitorBase<'a>,
}

impl<'a> SQLiteWallVisitor<'a> {
    /// 绑定 `SQLite` Provider。
    #[must_use]
    pub fn new(provider: &'a WallProvider) -> Self {
        Self {
            base: WallVisitorBase::new(provider),
        }
    }
}

impl WallVisitor for SQLiteWallVisitor<'_> {
    fn db_type(&self) -> DbType {
        // Java SQLiteWallVisitor#getDbType 历史上返回 postgresql；保留该可观察
        // Visitor 结果，Provider 自身仍使用 sqlite parser 方言。
        DbType::PostgreSql
    }
    fn provider(&self) -> &WallProvider {
        self.base.provider()
    }
    fn check(&mut self, _sql: &str, statements: &[Statement]) {
        self.base.check_common(statements);
    }
    fn violations(&self) -> &[WallViolation] {
        self.base.violations()
    }
    fn add_violation(&mut self, violation: WallViolation) {
        self.base.push_unique(violation);
    }
    fn sql_modified(&self) -> bool {
        self.base.sql_modified()
    }
    fn set_sql_modified(&mut self, sql_modified: bool) {
        self.base.set_sql_modified(sql_modified);
    }
    fn sql_end_of_comment(&self) -> bool {
        self.base.sql_end_of_comment()
    }
    fn set_sql_end_of_comment(&mut self, sql_end_of_comment: bool) {
        self.base.set_sql_end_of_comment(sql_end_of_comment);
    }
    fn add_wall_update_check_item(&mut self, item: WallUpdateCheckItem) {
        self.base.add_wall_update_check_item(item);
    }
    fn update_check_items(&self) -> Option<&[WallUpdateCheckItem]> {
        self.base.update_check_items()
    }
}
