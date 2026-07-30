//! 对应 Java：`com.alibaba.druid.wall.WallVisitor`。

use super::{DbType, WallConfig, WallProvider, WallUpdateCheckItem, WallViolation};
use sqlparser::ast::Statement;

/// Wall 方言 Visitor 的统一协议。
///
/// sqlparser-rs 使用枚举 AST 而非 Java 的多层 Visitor 接口，因此本协议保留
/// Provider、配置、违规、SQL 修改、尾注释和 UPDATE 检查项这些可观察状态，
/// 由各方言对象实现额外检查。
pub trait WallVisitor {
    /// 返回当前方言。
    fn db_type(&self) -> DbType;

    /// 返回所属 Provider。
    fn provider(&self) -> &WallProvider;

    /// 返回 Provider 配置。
    fn config(&self) -> &WallConfig {
        self.provider().config()
    }

    /// 检查已解析语句。
    fn check(&mut self, sql: &str, statements: &[Statement]);

    /// 返回累计违规。
    fn violations(&self) -> &[WallViolation];

    /// 增加违规。
    fn add_violation(&mut self, violation: WallViolation);

    /// 返回 SQL 是否被 Visitor 修改。
    fn sql_modified(&self) -> bool;

    /// 设置 SQL 修改状态。
    fn set_sql_modified(&mut self, sql_modified: bool);

    /// 返回 lexer 是否在注释末尾结束。
    fn sql_end_of_comment(&self) -> bool;

    /// 设置 lexer 尾注释状态。
    fn set_sql_end_of_comment(&mut self, sql_end_of_comment: bool);

    /// 增加 UPDATE 检查项。
    fn add_wall_update_check_item(&mut self, item: WallUpdateCheckItem);

    /// 返回 UPDATE 检查项；未创建时保持 `None`。
    fn update_check_items(&self) -> Option<&[WallUpdateCheckItem]>;
}
