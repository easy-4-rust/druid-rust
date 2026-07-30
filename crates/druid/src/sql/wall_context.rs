//! 对应 Java：`com.alibaba.druid.wall.WallContext`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/wall/WallContext.java`。

use super::{DbType, WallSqlFunctionStat, WallSqlStat, WallSqlTableStat, WallUpdateCheckItem};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

thread_local! {
    static WALL_CONTEXT: RefCell<Option<Arc<Mutex<WallContext>>>> =
        const { RefCell::new(None) };
}

/// 一次同步 Wall 检查的线程局部上下文。
///
/// Java 对象使用 `ThreadLocal`，WallFilter 会在同步检查前创建、finally 中清除。
/// Rust 同样只在不跨 `.await` 的检查区间使用线程局部槽；共享句柄允许 provider、
/// visitor 与 filter 观察同一个可变上下文，而不会暴露 JVM 对象模型。
pub struct WallContext {
    sql_stat: Option<Arc<WallSqlStat>>,
    table_stats: Option<HashMap<String, WallSqlTableStat>>,
    function_stats: Option<HashMap<String, WallSqlFunctionStat>>,
    db_type: DbType,
    comment_count: u32,
    warnings: u32,
    union_warnings: u32,
    update_none_condition_warnings: u32,
    delete_none_condition_warnings: u32,
    like_number_warnings: u32,
    wall_update_check_items: Option<Vec<WallUpdateCheckItem>>,
}

impl WallContext {
    /// 创建未绑定 SQL 统计的上下文值。
    #[must_use]
    pub fn new(db_type: DbType) -> Self {
        Self {
            sql_stat: None,
            table_stats: None,
            function_stats: None,
            db_type,
            comment_count: 0,
            warnings: 0,
            union_warnings: 0,
            update_none_condition_warnings: 0,
            delete_none_condition_warnings: 0,
            like_number_warnings: 0,
            wall_update_check_items: None,
        }
    }

    /// 当前线程不存在上下文时创建；已存在时保留原对象和原 dbType。
    ///
    /// 对应 Java：`WallContext#createIfNotExists(DbType)`。
    pub fn create_if_not_exists(db_type: DbType) -> Arc<Mutex<Self>> {
        WALL_CONTEXT.with(|slot| {
            let mut slot = slot.borrow_mut();
            Arc::clone(slot.get_or_insert_with(|| Arc::new(Mutex::new(Self::new(db_type)))))
        })
    }

    /// 创建并替换当前线程上下文。
    ///
    /// 对应 Java：`WallContext#create(DbType)`。
    pub fn create(db_type: DbType) -> Arc<Mutex<Self>> {
        let context = Arc::new(Mutex::new(Self::new(db_type)));
        Self::set_context(Some(Arc::clone(&context)));
        context
    }

    /// 返回当前线程上下文。
    #[must_use]
    pub fn current() -> Option<Arc<Mutex<Self>>> {
        WALL_CONTEXT.with(|slot| slot.borrow().clone())
    }

    /// 清除当前线程上下文。
    pub fn clear_context() {
        Self::set_context(None);
    }

    /// 设置或清除当前线程上下文。
    pub fn set_context(context: Option<Arc<Mutex<Self>>>) {
        WALL_CONTEXT.with(|slot| *slot.borrow_mut() = context);
    }

    /// 返回当前 SQL 统计对象。
    #[must_use]
    pub fn sql_stat(&self) -> Option<&Arc<WallSqlStat>> {
        self.sql_stat.as_ref()
    }

    /// 绑定当前 SQL 统计对象。
    pub fn set_sql_stat(&mut self, sql_stat: Option<Arc<WallSqlStat>>) {
        self.sql_stat = sql_stat;
    }

    /// 返回表级解析统计；尚未访问表时返回 `None`。
    #[must_use]
    pub fn table_stats(&self) -> Option<&HashMap<String, WallSqlTableStat>> {
        self.table_stats.as_ref()
    }

    /// 返回函数解析统计；尚未访问函数时返回 `None`。
    #[must_use]
    pub fn function_stats(&self) -> Option<&HashMap<String, WallSqlFunctionStat>> {
        self.function_stats.as_ref()
    }

    /// 返回 Wall 方言。
    #[must_use]
    pub const fn db_type(&self) -> DbType {
        self.db_type
    }

    /// 取得或创建表统计。
    ///
    /// 对应 Java `getTableStat` 的原始大小写 key 历史行为：查找使用小写，
    /// 新值却使用调用方原名插入；容量大于 100 后拒绝新项。
    pub fn table_stat(&mut self, table_name: &str) -> Option<&mut WallSqlTableStat> {
        let lower_case_name = table_name.to_lowercase();
        let stats = self
            .table_stats
            .get_or_insert_with(|| HashMap::with_capacity(2));
        if stats.contains_key(&lower_case_name) {
            return stats.get_mut(&lower_case_name);
        }
        if stats.len() > 100 {
            return None;
        }
        stats.insert(table_name.to_owned(), WallSqlTableStat::default());
        stats.get_mut(table_name)
    }

    /// 增加函数调用次数。
    ///
    /// 对应 Java：`WallContext#incrementFunctionInvoke(String)`。
    pub fn increment_function_invoke(&mut self, function_name: &str) {
        let lower_case_name = function_name.to_lowercase();
        let stats = self.function_stats.get_or_insert_with(HashMap::new);
        if let Some(stat) = stats.get_mut(&lower_case_name) {
            stat.increment_invoke_count();
            return;
        }
        if stats.len() > 100 {
            return;
        }
        stats.insert(function_name.to_owned(), WallSqlFunctionStat::default());
        if let Some(stat) = stats.get_mut(function_name) {
            stat.increment_invoke_count();
        }
    }

    /// 返回注释数量。
    #[must_use]
    pub const fn comment_count(&self) -> u32 {
        self.comment_count
    }

    /// 增加注释数量；首次注释同时增加总 warning。
    pub fn increment_comment_count(&mut self) {
        if self.comment_count == 0 {
            self.warnings = self.warnings.wrapping_add(1);
        }
        self.comment_count = self.comment_count.wrapping_add(1);
    }

    /// 返回总 warning 数。
    #[must_use]
    pub const fn warnings(&self) -> u32 {
        self.warnings
    }

    /// 增加总 warning。
    pub fn increment_warnings(&mut self) {
        self.warnings = self.warnings.wrapping_add(1);
    }

    /// 返回 LIKE-number warning 数。
    #[must_use]
    pub const fn like_number_warnings(&self) -> u32 {
        self.like_number_warnings
    }

    /// 增加 LIKE-number warning；首次同时增加总 warning。
    pub fn increment_like_number_warnings(&mut self) {
        if self.like_number_warnings == 0 {
            self.increment_warnings();
        }
        self.like_number_warnings = self.like_number_warnings.wrapping_add(1);
    }

    /// 返回 UNION warning 数。
    #[must_use]
    pub const fn union_warnings(&self) -> u32 {
        self.union_warnings
    }

    /// 增加 UNION warning；首次同时增加总 warning。
    pub fn increment_union_warnings(&mut self) {
        if self.union_warnings == 0 {
            self.increment_warnings();
        }
        self.union_warnings = self.union_warnings.wrapping_add(1);
    }

    /// 返回无条件 UPDATE warning 数。
    #[must_use]
    pub const fn update_none_condition_warnings(&self) -> u32 {
        self.update_none_condition_warnings
    }

    /// 增加无条件 UPDATE warning；Java 历史实现不增加总 warning。
    pub fn increment_update_none_condition_warnings(&mut self) {
        self.update_none_condition_warnings = self.update_none_condition_warnings.wrapping_add(1);
    }

    /// 返回无条件 DELETE warning 数。
    #[must_use]
    pub const fn delete_none_condition_warnings(&self) -> u32 {
        self.delete_none_condition_warnings
    }

    /// 增加无条件 DELETE warning；Java 历史实现不增加总 warning。
    pub fn increment_delete_none_condition_warnings(&mut self) {
        self.delete_none_condition_warnings = self.delete_none_condition_warnings.wrapping_add(1);
    }

    /// 返回 UPDATE 检查项。
    #[must_use]
    pub fn wall_update_check_items(&self) -> Option<&[WallUpdateCheckItem]> {
        self.wall_update_check_items.as_deref()
    }

    /// 设置 UPDATE 检查项；`None` 与空列表保持不同。
    pub fn set_wall_update_check_items(
        &mut self,
        wall_update_check_items: Option<Vec<WallUpdateCheckItem>>,
    ) {
        self.wall_update_check_items = wall_update_check_items;
    }

    pub(crate) fn replace_sql_stats(
        &mut self,
        table_stats: HashMap<String, WallSqlTableStat>,
        function_stats: HashMap<String, WallSqlFunctionStat>,
    ) {
        self.table_stats = Some(table_stats);
        self.function_stats = Some(function_stats);
    }
}
