//! Differential tests for `WallProvider` cache, stats, tenant and privileged paths.
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 目标：覆盖 `wall_provider.rs` 中白/黑名单缓存命中、语法错误计数、
//! violation `计数、统计快照、clear_cache、tenant_value、do_privileged` 等路径。

extern crate druid_core as druid;
use druid_core::core::Value;
use druid_core::sql::{
    WallConfig, WallProvider, WallViolation,
};


// ══════════════════════════════════════════════════════════════════
// 1. 白名单缓存命中路径
// ══════════════════════════════════════════════════════════════════

/// 第一次 check 做 hard check，第二次命中白名单缓存。
#[test]
fn white_list_cache_hit_on_second_check() {
    let provider = WallProvider::new(WallConfig::default());
    let r1 = provider.try_check("SELECT 1").unwrap();
    assert!(r1.violations().is_empty());
    assert_eq!(provider.hard_check_count(), 1);
    assert_eq!(provider.white_list_hit_count(), 0);

    let r2 = provider.try_check("SELECT 1").unwrap();
    assert!(r2.violations().is_empty());
    assert_eq!(provider.hard_check_count(), 1); // 未增加
    assert_eq!(provider.white_list_hit_count(), 1); // 命中白名单
    assert_eq!(provider.check_count(), 2);
}

/// 白名单 SQL 多次命中累积执行次数。
#[test]
fn white_list_execute_count_accumulates() {
    let provider = WallProvider::new(WallConfig::default());
    for _ in 0..5 {
        let _ = provider.try_check("SELECT 1");
    }
    let stat = provider.sql_stat("SELECT 1").unwrap();
    assert_eq!(stat.stat_value(false).execute_count, 5);
}

/// 白名单缓存命中时语法错误标志也会传递。
#[test]
fn white_list_hit_with_syntax_error_flag() {
    let provider = WallProvider::new(WallConfig::default());
    // 语法错误的 SQL 会进入黑名单（有 violation），不会进入白名单
    let _ = provider.try_check("SELCT * FORM users");
    assert_eq!(provider.syntax_error_count(), 1);
    // 再次检查同一条错误 SQL：命中黑名单缓存
    let _ = provider.try_check("SELCT * FORM users");
    assert_eq!(provider.syntax_error_count(), 2); // 黑名单命中时也累加
}

// ══════════════════════════════════════════════════════════════════
// 2. 黑名单缓存命中路径
// ══════════════════════════════════════════════════════════════════

/// 违规 SQL 进入黑名单，第二次命中黑名单缓存。
#[test]
fn black_list_cache_hit_on_violation() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("DROP TABLE users");
    assert_eq!(provider.violation_count(), 1);
    assert_eq!(provider.hard_check_count(), 1);
    assert_eq!(provider.black_list_hit_count(), 0);

    // 第二次命中黑名单
    let _ = provider.try_check("DROP TABLE users");
    assert_eq!(provider.violation_count(), 2);
    assert_eq!(provider.hard_check_count(), 1); // 未增加
    assert_eq!(provider.black_list_hit_count(), 1);
}

/// 黑名单 SQL 的 violations 在缓存命中时保留。
#[test]
fn black_list_hit_preserves_violations() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let r1 = provider.try_check("DROP TABLE users").unwrap();
    assert!(!r1.violations().is_empty());
    let r2 = provider.try_check("DROP TABLE users").unwrap();
    assert!(!r2.violations().is_empty());
    assert!(r2
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DropTableNotAllowed(_))));
}

// ══════════════════════════════════════════════════════════════════
// 3. 语法错误计数
// ══════════════════════════════════════════════════════════════════

/// 语法错误 SQL 增加 `syntax_error_count`。
#[test]
fn syntax_error_count_increments() {
    let provider = WallProvider::new(WallConfig::default());
    assert_eq!(provider.syntax_error_count(), 0);
    let _ = provider.try_check("NOT VALID SQL !!!");
    assert_eq!(provider.syntax_error_count(), 1);
    let _ = provider.try_check("ALSO INVALID $$$");
    assert_eq!(provider.syntax_error_count(), 2);
}

/// 合法 SQL 不增加 `syntax_error_count`。
#[test]
fn valid_sql_no_syntax_error() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    assert_eq!(provider.syntax_error_count(), 0);
}

// ══════════════════════════════════════════════════════════════════
// 4. violation 计数
// ══════════════════════════════════════════════════════════════════

/// 违规 SQL 增加 `violation_count`。
#[test]
fn violation_count_increments() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    assert_eq!(provider.violation_count(), 0);
    let _ = provider.try_check("DROP TABLE t1");
    assert_eq!(provider.violation_count(), 1);
    let _ = provider.try_check("DROP TABLE t2");
    assert_eq!(provider.violation_count(), 2);
}

/// 合法 SQL 不增加 `violation_count`。
#[test]
fn valid_sql_no_violation() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    assert_eq!(provider.violation_count(), 0);
}

// ══════════════════════════════════════════════════════════════════
// 5. hard_check_count
// ══════════════════════════════════════════════════════════════════

/// 每次 SQL 不在缓存中时 `hard_check_count` 增加。
#[test]
fn hard_check_count_only_on_cache_miss() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    assert_eq!(provider.hard_check_count(), 1);
    let _ = provider.try_check("SELECT 2"); // 不同 SQL → cache miss
    assert_eq!(provider.hard_check_count(), 2);
    let _ = provider.try_check("SELECT 1"); // 命中白名单 → 不增加
    assert_eq!(provider.hard_check_count(), 2);
}

// ══════════════════════════════════════════════════════════════════
// 6. stat_value 快照
// ══════════════════════════════════════════════════════════════════

/// `stat_value(false)` 返回当前计数不重置。
#[test]
fn stat_value_no_reset() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let sv = provider.stat_value(false);
    assert_eq!(sv.check_count, 1);
    assert_eq!(sv.hard_check_count, 1);
    assert_eq!(sv.violation_count, 0);
    assert_eq!(sv.syntax_error_count, 0);
    // 再次查询，计数应继续累积
    let _ = provider.try_check("SELECT 2");
    let sv2 = provider.stat_value(false);
    assert_eq!(sv2.check_count, 2);
}

/// `stat_value(true)` 返回当前计数并重置。
#[test]
fn stat_value_with_reset() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let sv = provider.stat_value(true);
    assert_eq!(sv.check_count, 1);
    // 重置后计数归零
    assert_eq!(provider.check_count(), 0);
    assert_eq!(provider.hard_check_count(), 0);
}

/// `stat_value` 包含 `violation_effect_row_count`。
#[test]
fn stat_value_violation_effect_row_count() {
    let provider = WallProvider::new(WallConfig::default());
    provider.add_violation_effect_row_count(100);
    provider.add_violation_effect_row_count(50);
    let sv = provider.stat_value(false);
    assert_eq!(sv.violation_effect_row_count, 150);
}

/// `stats_map` 返回管理字段映射。
#[test]
fn stats_map_contains_keys() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let map = provider.stats_map();
    assert!(map.contains_key("checkCount"));
    assert!(map.contains_key("hardCheckCount"));
    assert!(map.contains_key("violationCount"));
    assert!(map.contains_key("whiteListHitCount"));
    assert!(map.contains_key("blackListHitCount"));
    assert!(map.contains_key("syntaxErrorCount"));
}

/// `stat_value` 白名单过滤 `execute_count==0` 的 SQL。
#[test]
fn stat_value_filters_empty_entries() {
    let provider = WallProvider::new(WallConfig::default());
    let sv = provider.stat_value(false);
    assert!(sv.white_list.is_empty());
    assert!(sv.black_list.is_empty());
}

/// `stat_value` 包含有执行数据的 SQL。
#[test]
fn stat_value_includes_active_sql() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let sv = provider.stat_value(false);
    assert!(!sv.white_list.is_empty());
    assert!(sv.white_list.iter().any(|s| s.execute_count > 0));
}

// ══════════════════════════════════════════════════════════════════
// 7. clear_cache
// ══════════════════════════════════════════════════════════════════

/// `clear_cache` 清空白/黑名单但保留计数。
#[test]
fn clear_cache_preserves_counts() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    assert!(!provider.white_list().is_empty());
    provider.clear_cache();
    assert!(provider.white_list().is_empty());
    assert!(provider.black_list().is_empty());
    // 计数不受影响
    assert_eq!(provider.check_count(), 1);
}

// ══════════════════════════════════════════════════════════════════
// 8. reset
// ══════════════════════════════════════════════════════════════════

/// reset 清零所有计数、缓存和统计。
#[test]
fn reset_clears_all() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT 1");
    let _ = provider.try_check("DROP TABLE t");
    assert!(provider.check_count() > 0);
    assert!(!provider.white_list().is_empty() || !provider.black_list().is_empty());
    provider.reset();
    assert_eq!(provider.check_count(), 0);
    assert_eq!(provider.hard_check_count(), 0);
    assert_eq!(provider.violation_count(), 0);
    assert_eq!(provider.white_list_hit_count(), 0);
    assert_eq!(provider.black_list_hit_count(), 0);
    assert_eq!(provider.syntax_error_count(), 0);
    assert_eq!(provider.violation_effect_row_count(), 0);
    assert!(provider.white_list().is_empty());
    assert!(provider.black_list().is_empty());
}

// ══════════════════════════════════════════════════════════════════
// 9. white_list / black_list / sql_stat
// ══════════════════════════════════════════════════════════════════

/// `white_list` 返回白名单 SQL 集合。
#[test]
fn white_list_returns_set() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let wl = provider.white_list();
    assert!(wl.contains("SELECT 1"));
}

/// `black_list` 返回黑名单 SQL 集合。
#[test]
fn black_list_returns_set() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("DROP TABLE t");
    let bl = provider.black_list();
    assert!(bl.contains("DROP TABLE t"));
}

/// `sql_stat` 查询白名单。
#[test]
fn sql_stat_from_white_list() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let stat = provider.sql_stat("SELECT 1");
    assert!(stat.is_some());
}

/// `sql_stat` 查询黑名单。
#[test]
fn sql_stat_from_black_list() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("DROP TABLE t");
    let stat = provider.sql_stat("DROP TABLE t");
    assert!(stat.is_some());
}

/// `sql_stat` 不存在的 SQL。
#[test]
fn sql_stat_not_found() {
    let provider = WallProvider::new(WallConfig::default());
    assert!(provider.sql_stat("SELECT 999").is_none());
}

// ══════════════════════════════════════════════════════════════════
// 10. table_stat_values / function_stat_values / sql_stat_values
// ══════════════════════════════════════════════════════════════════

/// SELECT 语句的表统计。
#[test]
fn table_stat_values_from_select() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT * FROM users WHERE id = 1");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// INSERT 语句的表统计。
#[test]
fn table_stat_values_from_insert() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("INSERT INTO orders (id) VALUES (1)");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "orders"));
}

/// UPDATE 语句的表统计。
#[test]
fn table_stat_values_from_update() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("UPDATE users SET name = 'a' WHERE id = 1");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// DELETE 语句的表统计。
#[test]
fn table_stat_values_from_delete() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("DELETE FROM users WHERE id = 1");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// 函数统计。
#[test]
fn function_stat_values_from_select() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT COUNT(*) FROM users");
    let functions = provider.function_stat_values(false);
    assert!(functions.iter().any(|f| f.name == "count"));
}

/// `sql_stat_values` 包含白/黑 SQL。
#[test]
fn sql_stat_values_combined() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT 1");
    let _ = provider.try_check("DROP TABLE t");
    let values = provider.sql_stat_values(false);
    assert!(values.len() >= 2);
}

/// `white_list_values` 只包含白名单。
#[test]
fn white_list_values_only() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT 1");
    let values = provider.white_list_values(false);
    assert!(!values.is_empty());
}

/// `black_list_values` 只包含黑名单。
#[test]
fn black_list_values_only() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("DROP TABLE t");
    let values = provider.black_list_values(false);
    assert!(!values.is_empty());
}

// ══════════════════════════════════════════════════════════════════
// 11. record_effect_rows
// ══════════════════════════════════════════════════════════════════

/// `record_effect_rows` 回写表统计行数。
#[test]
fn record_effect_rows_updates_table_stat() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("INSERT INTO orders (id) VALUES (1)");
    provider.record_effect_rows("INSERT INTO orders (id) VALUES (1)", 10, None);
    let tables = provider.table_stat_values(false);
    let orders = tables.iter().find(|t| t.name == "orders");
    assert!(orders.is_some());
}

/// `record_effect_rows` 无缓存命中时安全跳过。
#[test]
fn record_effect_rows_no_stat_skips() {
    let provider = WallProvider::new(WallConfig::default());
    // 未 check 过的 SQL → 无 stat → 安全跳过
    provider.record_effect_rows("SELECT 1", 10, Some(10));
}

/// `record_effect_rows` `路径（select_count` > 0）。
#[test]
fn record_effect_rows_select_fetch() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT * FROM users WHERE id = 1");
    provider.record_effect_rows("SELECT * FROM users WHERE id = 1", 0, Some(50));
    let tables = provider.table_stat_values(false);
    let users = tables.iter().find(|t| t.name == "users");
    assert!(users.is_some());
}

// ══════════════════════════════════════════════════════════════════
// 12. tenant_value / set_tenant_value
// ══════════════════════════════════════════════════════════════════

/// 线程级 `tenant_value` 设置与获取。
#[test]
fn thread_tenant_value_set_and_get() {
    assert!(WallProvider::tenant_value().is_none());
    WallProvider::set_tenant_value(Some(Value::String("tenant-1".to_owned())));
    let v = WallProvider::tenant_value();
    assert!(v.is_some());
    match v.unwrap() {
        Value::String(s) => assert_eq!(s, "tenant-1"),
        other => panic!("expected String, got {other:?}"),
    }
    WallProvider::set_tenant_value(None);
    assert!(WallProvider::tenant_value().is_none());
}

/// `tenant_value` 在不同线程独立。
#[test]
fn tenant_value_thread_isolation() {
    WallProvider::set_tenant_value(Some(Value::String("main-thread".to_owned())));
    let handle = std::thread::spawn(|| {
        // 新线程应为 None
        assert!(WallProvider::tenant_value().is_none());
        WallProvider::set_tenant_value(Some(Value::String("child-thread".to_owned())));
        let v = WallProvider::tenant_value();
        match v.unwrap() {
            Value::String(s) => assert_eq!(s, "child-thread"),
            other => panic!("expected String, got {other:?}"),
        }
    });
    handle.join().unwrap();
    // 主线程不受影响
    let v = WallProvider::tenant_value();
    match v.unwrap() {
        Value::String(s) => assert_eq!(s, "main-thread"),
        other => panic!("expected String, got {other:?}"),
    }
    WallProvider::set_tenant_value(None);
}

// ══════════════════════════════════════════════════════════════════
// 13. scope_tenant_value（Tokio task-local）
// ══════════════════════════════════════════════════════════════════

/// `scope_tenant_value` 在作用域内设置、退出后恢复。
#[tokio::test]
async fn scope_tenant_value_sets_and_restores() {
    WallProvider::set_tenant_value(None);
    WallProvider::scope_tenant_value(Value::String("scoped".to_owned()), async {
        let v = WallProvider::tenant_value();
        match v.unwrap() {
            Value::String(s) => assert_eq!(s, "scoped"),
            other => panic!("expected String, got {other:?}"),
        }
    })
    .await;
    // 退出后恢复为 None
    assert!(WallProvider::tenant_value().is_none());
}

/// `scope_tenant_value` 嵌套作用域。
#[tokio::test]
async fn scope_tenant_value_nested() {
    WallProvider::scope_tenant_value(Value::Int(1), async {
        let v1 = WallProvider::tenant_value();
        assert!(matches!(v1.unwrap(), Value::Int(1)));

        WallProvider::scope_tenant_value(Value::Int(2), async {
            let v2 = WallProvider::tenant_value();
            assert!(matches!(v2.unwrap(), Value::Int(2)));
        })
        .await;

        // 退出内层后恢复外层
        let v1 = WallProvider::tenant_value();
        assert!(matches!(v1.unwrap(), Value::Int(1)));
    })
    .await;
}

// ══════════════════════════════════════════════════════════════════
// 14. is_privileged / do_privileged
// ══════════════════════════════════════════════════════════════════

/// 默认非 privileged。
#[test]
fn not_privileged_by_default() {
    assert!(!WallProvider::is_privileged());
}

/// `do_privileged` 内部为 true，退出后恢复。
#[test]
fn do_privileged_sets_and_restores() {
    assert!(!WallProvider::is_privileged());
    let result = WallProvider::do_privileged(|| {
        assert!(WallProvider::is_privileged());
        42
    });
    assert_eq!(result, 42);
    assert!(!WallProvider::is_privileged());
}

/// `do_privileged` 嵌套。
#[test]
fn do_privileged_nested() {
    assert!(!WallProvider::is_privileged());
    WallProvider::do_privileged(|| {
        assert!(WallProvider::is_privileged());
        WallProvider::do_privileged(|| {
            assert!(WallProvider::is_privileged());
        });
        assert!(WallProvider::is_privileged());
    });
    assert!(!WallProvider::is_privileged());
}

/// `do_privileged` 恢复即使闭包 panic。
#[test]
fn do_privileged_restores_on_panic() {
    assert!(!WallProvider::is_privileged());
    let result = std::panic::catch_unwind(|| {
        WallProvider::do_privileged(|| {
            assert!(WallProvider::is_privileged());
            panic!("intentional");
        });
    });
    assert!(result.is_err());
    assert!(!WallProvider::is_privileged());
}

// ══════════════════════════════════════════════════════════════════
// 15. scope_privileged（Tokio task-local）
// ══════════════════════════════════════════════════════════════════

/// `scope_privileged` 在作用域内启用，退出后恢复。
#[tokio::test]
async fn scope_privileged_sets_and_restores() {
    assert!(!WallProvider::is_privileged());
    WallProvider::scope_privileged(async {
        assert!(WallProvider::is_privileged());
    })
    .await;
    assert!(!WallProvider::is_privileged());
}

// ══════════════════════════════════════════════════════════════════
// 16. privileged check 绕过 wall 检查
// ══════════════════════════════════════════════════════════════════

/// `do_privileged_allow=true` + `is_privileged` → 快速通行。
#[test]
fn privileged_bypass_wall_check() {
    let mut config = WallConfig::default();
    config.do_privileged_allow = true;
    let provider = WallProvider::new(config);
    let result = WallProvider::do_privileged(|| {
        let r = provider.try_check("DROP TABLE users").unwrap();
        assert!(r.violations().is_empty());
        assert!(r.sql_stat().is_none());
        r
    });
    assert_eq!(result.sql(), "DROP TABLE users");
}

/// `do_privileged_allow=false` → privileged 不绕过。
#[test]
fn privileged_no_bypass_when_disabled() {
    let config = WallConfig::builder()
        .do_privileged_allow(false)
        .drop_table_allow(false)
        .build();
    let provider = WallProvider::new(config);
    WallProvider::do_privileged(|| {
        let r = provider.try_check("DROP TABLE users").unwrap();
        assert!(!r.violations().is_empty());
    });
}

// ══════════════════════════════════════════════════════════════════
// 17. 多 tenant SQL 缓存绕过
// ══════════════════════════════════════════════════════════════════

/// `tenant_table_pattern` 非空时绕过白/黑名单缓存。
#[test]
fn tenant_pattern_disables_cache() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t*")
        .tenant_column("tenant_id")
        .build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT * FROM t_orders WHERE id = 1");
    let _ = provider.try_check("SELECT * FROM t_orders WHERE id = 1");
    // 两次都做 hard check（无缓存命中）
    assert_eq!(provider.hard_check_count(), 2);
}

// ══════════════════════════════════════════════════════════════════
// 18. normalize_name 覆盖（反引号/双引号剥离）
// ══════════════════════════════════════════════════════════════════

/// 反引号表名被标准化为小写。
#[test]
fn backtick_table_name_normalized() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT * FROM `Users` WHERE id = 1");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// 双引号表名被标准化为小写。
#[test]
fn double_quote_table_name_normalized() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check(r#"SELECT * FROM "Users" WHERE id = 1"#);
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

// ══════════════════════════════════════════════════════════════════
// 25. collect_sql_stats 覆盖
// ══════════════════════════════════════════════════════════════════

/// TRUNCATE 语句表统计。
#[test]
fn table_stat_truncate() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("TRUNCATE TABLE users");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// CREATE TABLE 语句表统计。
#[test]
fn table_stat_create_table() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("CREATE TABLE users (id INT)");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// ALTER TABLE 语句表统计。
#[test]
fn table_stat_alter_table() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("ALTER TABLE users ADD COLUMN age INT");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// DROP TABLE 语句表统计。
#[test]
fn table_stat_drop_table() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("DROP TABLE users");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "users"));
}

/// INSERT ... SELECT 子查询表统计。
#[test]
fn table_stat_insert_select() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("INSERT INTO orders SELECT * FROM staging");
    let tables = provider.table_stat_values(false);
    assert!(tables.iter().any(|t| t.name == "orders"));
    assert!(tables.iter().any(|t| t.name == "staging"));
}

/// 多函数统计。
#[test]
fn function_stat_multiple() {
    let provider = WallProvider::new(WallConfig::default());
    let _ = provider.try_check("SELECT COUNT(*), MAX(id) FROM users");
    let functions = provider.function_stat_values(false);
    assert!(functions.iter().any(|f| f.name == "count"));
    assert!(functions.iter().any(|f| f.name == "max"));
}
