//! 对应 Java：`com.alibaba.druid.wall.spi.MySqlWallVisitor`。

use crate::sql::{
    DbType, WallProvider, WallUpdateCheckItem, WallViolation, WallVisitor, WallVisitorBase,
};
use sqlparser::ast::Statement;

/// MySQL Wall 方言 Visitor。
pub struct MySqlWallVisitor<'a> {
    base: WallVisitorBase<'a>,
}

impl<'a> MySqlWallVisitor<'a> {
    /// 绑定 MySQL Provider。
    #[must_use]
    pub fn new(provider: &'a WallProvider) -> Self {
        Self {
            base: WallVisitorBase::new(provider),
        }
    }
}

impl WallVisitor for MySqlWallVisitor<'_> {
    fn db_type(&self) -> DbType {
        DbType::MySql
    }

    fn provider(&self) -> &WallProvider {
        self.base.provider()
    }

    fn check(&mut self, sql: &str, statements: &[Statement]) {
        self.base.check_common(statements);
        self.base.check_deny_variants(statements);
        if !self.config().select_into_outfile_allow && contains_into_outfile(sql) {
            self.base
                .push_unique(WallViolation::SelectIntoOutfileNotAllowed);
        }
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

fn contains_into_outfile(sql: &str) -> bool {
    let words = sql
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    words
        .windows(2)
        .any(|words| words[0] == "into" && words[1] == "outfile")
}
