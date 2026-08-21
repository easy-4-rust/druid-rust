//! `WallConfig` 全字段行为接线差分测试（C6 Step 8）。
//!
//! 对照 Java 源：`WallVisitorUtils#preVisitCheck`（语句门控）、
//! `WallVisitorUtils#getConditionValue/getValue_and`（条件语义族）、
//! `MySqlWallVisitor#isDeny`（变量拒绝）、`WallVisitorUtils#checkReadOnly`、
//! `checkFunction`（functionCheck 门）、`checkSchema/checkTable`（schema/table 门）。
//! 每条断言记录 Java 默认值与行为来源。

use druid::sql::{DbType, Wall, WallConfig, WallViolation};

fn wall_with(config: WallConfig) -> Wall {
    Wall::with_db_type(config, DbType::MySql)
}

fn violations_of(config: WallConfig, sql: &str) -> Vec<WallViolation> {
    match wall_with(config).check(sql) {
        Ok(()) => Vec::new(),
        Err(violations) => violations,
    }
}

// ── preVisitCheck 语句门控 ─────────────────────────────────────

/// Java：`SQLUseStatement → useAllow`（默认 `true，ErrorCode.USE_NOT_ALLOW` 1203）。
#[test]
fn use_statement_blocked_when_disabled() {
    let blocked = violations_of(WallConfig::builder().use_allow(false).build(), "USE mydb");
    assert_eq!(
        blocked,
        vec![WallViolation::OperationNotAllowed("USE".to_owned())]
    );
    let allowed = violations_of(WallConfig::default(), "USE mydb");
    assert!(allowed.is_empty());
}

/// Java：`SQLShowStatement 族 → showAllow`（默认 `true，ErrorCode.SHOW_NOT_ALLOW` 1202）。
#[test]
fn show_statements_blocked_when_disabled() {
    for sql in [
        "SHOW TABLES",
        "SHOW DATABASES",
        "SHOW COLUMNS FROM t",
        "SHOW SCHEMAS",
        "SHOW VIEWS",
    ] {
        let blocked = violations_of(WallConfig::builder().show_allow(false).build(), sql);
        assert_eq!(
            blocked,
            vec![WallViolation::OperationNotAllowed("SHOW".to_owned())],
            "sql={sql}"
        );
        let allowed = violations_of(WallConfig::default(), sql);
        assert!(allowed.is_empty(), "sql={sql} default should pass");
    }
}

/// Java：`SQLDescribeStatement → describeAllow`（默认 `true，ErrorCode.DESC_NOT_ALLOW` 1201）。
#[test]
fn describe_statements_blocked_when_disabled() {
    for sql in ["DESC t", "DESCRIBE t"] {
        let blocked = violations_of(WallConfig::builder().describe_allow(false).build(), sql);
        assert_eq!(
            blocked,
            vec![WallViolation::OperationNotAllowed("DESCRIBE".to_owned())],
            "sql={sql}"
        );
    }
    // EXPLAIN（非 describe 别名）在 Java 中 allow=true，不受 describeAllow 影响。
    let explain = violations_of(
        WallConfig::builder().describe_allow(false).build(),
        "EXPLAIN SELECT 1",
    );
    assert!(explain.is_empty());
}

/// Java：`SQLCallStatement → callAllow`（默认 `true，ErrorCode.CALL_NOT_ALLOW` 1300）。
#[test]
fn call_statement_blocked_when_disabled() {
    let blocked = violations_of(
        WallConfig::builder().call_allow(false).build(),
        "CALL do_something()",
    );
    assert_eq!(
        blocked,
        vec![WallViolation::OperationNotAllowed("CALL".to_owned())]
    );
}

/// Java：`SQLDropStatement → dropTableAllow` 门控所有 DROP 对象，不只 TABLE。
#[test]
fn drop_non_table_object_gated_by_drop_table_allow() {
    // MySQL DROP INDEX 解析形态不保证；使用 TRUNCATE 邻近语义验证非 Table 分支
    // 需要真实非 Table DROP：DROP VIEW 在 MySQL 方言下是 Drop { View }。
    let blocked = violations_of(
        WallConfig::builder().drop_table_allow(false).build(),
        "DROP VIEW v",
    );
    assert!(
        blocked.contains(&WallViolation::OperationNotAllowed("DROP VIEW".to_owned())),
        "violations={blocked:?}"
    );
}

/// Java：`SQLSetOperation INTERSECT → intersectAllow`（默认 true）。
#[test]
fn intersect_blocked_when_disabled() {
    let blocked = violations_of(
        WallConfig::builder().intersect_allow(false).build(),
        "SELECT 1 INTERSECT SELECT 2",
    );
    assert_eq!(
        blocked,
        vec![WallViolation::OperationNotAllowed("INTERSECT".to_owned())]
    );
}

// ── 条件语义族（getConditionValue/getValue_and）──────────────

/// Java：非首位恒假 part + `conditionAndAlwayFalseAllow=false`（默认）
/// → `ErrorCode.ALWAYS_FALSE（2113）"part` alway false condition not allow"。
#[test]
fn part_alway_false_denied_by_default() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE a = 1 AND 1 = 2",
    );
    assert!(
        violations.contains(&WallViolation::AlwaysFalseCondition("part".to_owned())),
        "violations={violations:?}"
    );
}

/// Java：非首位恒真 part 由 `conditionAndAlwayTrueAllow=true`（默认）放行。
#[test]
fn part_alway_true_allowed_by_default() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE a = 1 AND 2 = 2",
    );
    assert!(
        !violations.contains(&WallViolation::AlwaysTrueCondition("part".to_owned())),
        "violations={violations:?}"
    );
    // 关闭后必须拒绝
    let blocked = violations_of(
        WallConfig::builder()
            .condition_and_alway_true_allow(false)
            .build(),
        "SELECT * FROM t WHERE a = 1 AND 2 = 2",
    );
    assert!(
        blocked.contains(&WallViolation::AlwaysTrueCondition("part".to_owned())),
        "violations={blocked:?}"
    );
}

/// Java `getValue_and`：dalConst==2 且 `conditionDoubleConstAllow=false`（默认）
/// → `ErrorCode.DOUBLE_CONST_CONDITION（2107`）。
#[test]
fn double_const_condition_denied_by_default() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE 1 = 1 AND 2 = 2",
    );
    assert!(
        violations.contains(&WallViolation::DoubleConstCondition),
        "violations={violations:?}"
    );
    // 放开后允许
    let allowed = violations_of(
        WallConfig::builder()
            .condition_double_const_allow(true)
            .build(),
        "SELECT * FROM t WHERE 1 = 1 AND 2 = 2",
    );
    assert!(
        !allowed.contains(&WallViolation::DoubleConstCondition),
        "violations={allowed:?}"
    );
}

/// UPDATE / DELETE 的 WHERE 同样进入条件语义检查（Java checkUpdate/checkDelete）。
#[test]
fn condition_checks_apply_to_update_and_delete() {
    let update = violations_of(
        WallConfig::default(),
        "UPDATE t SET a = 1 WHERE b = 1 AND 1 = 2",
    );
    assert!(
        update.contains(&WallViolation::AlwaysFalseCondition("part".to_owned())),
        "violations={update:?}"
    );
    let delete = violations_of(WallConfig::default(), "DELETE FROM t WHERE b = 1 AND 1 = 2");
    assert!(
        delete.contains(&WallViolation::AlwaysFalseCondition("part".to_owned())),
        "violations={delete:?}"
    );
}

/// Java：XOR 运算符 + `conditionOpXorAllow=false`（默认）→ ErrorCode.XOR（2102）。
#[test]
fn xor_operator_denied_by_default() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE a = 1 XOR b = 2",
    );
    assert!(
        violations.contains(&WallViolation::XorNotAllowed),
        "{violations:?}"
    );
}

/// Java：位运算符 + `conditionOpBitwiseAllow=true`（默认）放行；关闭后
/// → ErrorCode.BITWISE（2103）。
#[test]
fn bitwise_operator_gated() {
    let allowed = violations_of(WallConfig::default(), "SELECT * FROM t WHERE (a & 1) = 1");
    assert!(
        !allowed.contains(&WallViolation::BitwiseNotAllowed),
        "violations={allowed:?}"
    );
    let blocked = violations_of(
        WallConfig::builder()
            .condition_op_bitwise_allow(false)
            .build(),
        "SELECT * FROM t WHERE (a & 1) = 1",
    );
    assert!(
        blocked.contains(&WallViolation::BitwiseNotAllowed),
        "{blocked:?}"
    );
}

/// Java：常量算术 + `constArithmeticAllow=true`（默认）放行；关闭后
/// → `ErrorCode.CONST_ARITHMETIC（2101`）。
#[test]
fn const_arithmetic_gated() {
    let allowed = violations_of(WallConfig::default(), "SELECT * FROM t WHERE a = 1 + 1");
    assert!(
        !allowed.contains(&WallViolation::ConstArithmeticNotAllowed),
        "violations={allowed:?}"
    );
    let blocked = violations_of(
        WallConfig::builder().const_arithmetic_allow(false).build(),
        "SELECT * FROM t WHERE a = 1 + 1",
    );
    assert!(
        blocked.contains(&WallViolation::ConstArithmeticNotAllowed),
        "{blocked:?}"
    );
}

/// Java：`LIKE` 两侧相同常量字符串 → `ErrorCode.SAME_CONST_LIKE（2108`）。
#[test]
fn same_const_like_denied() {
    let violations = violations_of(WallConfig::default(), "SELECT * FROM t WHERE 'a' LIKE 'a'");
    assert!(
        violations.contains(&WallViolation::SameConstLike),
        "{violations:?}"
    );
    // 非相同常量不触发
    let distinct = violations_of(WallConfig::default(), "SELECT * FROM t WHERE 'a' LIKE 'b'");
    assert!(
        !distinct.contains(&WallViolation::SameConstLike),
        "violations={distinct:?}"
    );
}

/// Java：`CASE WHEN` 常量条件 + `caseConditionConstAllow=false`（默认）
/// → `ErrorCode.CONST_CASE_CONDITION（2109`）。
#[test]
fn const_case_condition_denied_by_default() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE CASE WHEN 1 = 1 THEN true ELSE false END",
    );
    assert!(
        violations.contains(&WallViolation::ConstCaseCondition),
        "{violations:?}"
    );
    let allowed = violations_of(
        WallConfig::builder()
            .case_condition_const_allow(true)
            .build(),
        "SELECT * FROM t WHERE CASE WHEN 1 = 1 THEN true ELSE false END",
    );
    assert!(
        !allowed.contains(&WallViolation::ConstCaseCondition),
        "violations={allowed:?}"
    );
}

/// HAVING 子句同样进入条件语义检查（Java checkHaving → checkCondition）。
#[test]
fn condition_checks_apply_to_having() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT a, COUNT(*) FROM t GROUP BY a HAVING 1 = 1 AND 2 = 2",
    );
    assert!(
        violations.contains(&WallViolation::DoubleConstCondition),
        "violations={violations:?}"
    );
}

// ── 变量拒绝（MySqlWallVisitor#isDeny + WallVisitor#visit(SQLIdentifierExpr)）──

/// Java `isDeny`：`@@version` 去掉 `@@` 前缀后小写匹配 denyVariants。
/// 默认 `MySQL` 配置目录 deny-variant.txt 含 version/datadir。
#[test]
fn deny_variant_blocks_system_variable() {
    let config = WallConfig::builder().deny_variant("secret").build();
    let blocked = violations_of(config, "SELECT @@secret FROM t");
    assert!(
        blocked.contains(&WallViolation::DeniedVariant("@@secret".to_owned())),
        "violations={blocked:?}"
    );
    // 未列出的变量放行
    let config = WallConfig::builder().deny_variant("secret").build();
    let allowed = violations_of(config, "SELECT @other FROM t");
    assert!(
        !allowed
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedVariant(_))),
        "violations={allowed:?}"
    );
}

/// Java：`variantCheck=false` 时完全跳过变量检查。
#[test]
fn variant_check_gate_disables_variant_deny() {
    let mut config = WallConfig::default();
    config.variant_check = false;
    config.deny_variants.push("secret".to_owned());
    let violations = violations_of(config, "SELECT @@secret FROM t");
    assert!(
        !violations
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedVariant(_))),
        "violations={violations:?}"
    );
}

// ── function/schema/table/object 门 ──────────────────────────

/// Java `checkFunction`：`functionCheck=false` 时直接返回，不检查 denyFunctions。
#[test]
fn function_check_gate_disables_function_deny() {
    let mut gated = WallConfig::default();
    gated.function_check = false;
    gated.deny_functions.push("sleep".to_owned());
    let violations = violations_of(gated, "SELECT sleep(1) FROM t");
    assert!(
        !violations
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedFunction(_))),
        "violations={violations:?}"
    );
    let mut enabled = WallConfig::default();
    enabled.deny_functions.push("sleep".to_owned());
    let blocked = violations_of(enabled, "SELECT sleep(1) FROM t");
    assert!(
        blocked.contains(&WallViolation::DeniedFunction("sleep".to_owned())),
        "{blocked:?}"
    );
}

/// Java `checkSchema`：`schemaCheck=false` 时跳过 denySchema。
#[test]
fn schema_check_gate_disables_schema_deny() {
    let mut gated = WallConfig::default();
    gated.schema_check = false;
    gated.deny_schemas.push("secret".to_owned());
    let violations = violations_of(gated, "SELECT * FROM secret.t");
    assert!(
        !violations
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedSchema(_))),
        "violations={violations:?}"
    );
    let mut enabled = WallConfig::default();
    enabled.deny_schemas.push("secret".to_owned());
    let blocked = violations_of(enabled, "SELECT * FROM secret.t");
    assert!(
        blocked.contains(&WallViolation::DeniedSchema("secret".to_owned())),
        "{blocked:?}"
    );
}

/// Java `checkTable`：`tableCheck=false` 时跳过 denyTable。
#[test]
fn table_check_gate_disables_table_deny() {
    let mut gated = WallConfig::default();
    gated.table_check = false;
    gated.deny_tables.push("sensitive".to_owned());
    let violations = violations_of(gated, "SELECT * FROM sensitive");
    assert!(
        !violations
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedTable(_))),
        "violations={violations:?}"
    );
    let mut enabled = WallConfig::default();
    enabled.deny_tables.push("sensitive".to_owned());
    let blocked = violations_of(enabled, "SELECT * FROM sensitive");
    assert!(
        blocked.contains(&WallViolation::DeniedTable("sensitive".to_owned())),
        "{blocked:?}"
    );
}

/// Java denyObjects（objectCheck 门）：完整对象名匹配 → `OBJECT_DENY（2005`）。
#[test]
fn deny_object_blocks_full_object_name() {
    let mut config = WallConfig::default();
    config.deny_objects.push("mysql.user".to_owned());
    let blocked = violations_of(config, "SELECT * FROM mysql.user");
    assert!(
        blocked.contains(&WallViolation::DeniedObject("mysql.user".to_owned())),
        "{blocked:?}"
    );
    let mut gated = WallConfig::default();
    gated.object_check = false;
    gated.deny_objects.push("mysql.user".to_owned());
    let allowed = violations_of(gated, "SELECT * FROM mysql.user");
    assert!(
        !allowed
            .iter()
            .any(|v| matches!(v, WallViolation::DeniedObject(_))),
        "violations={allowed:?}"
    );
}

// ── 只读表（WallVisitorUtils#checkReadOnly + WallConfig#isReadOnly）──

/// Java：UPDATE/DELETE/INSERT/TRUNCATE 命中 readOnlyTables → `READ_ONLY（4000`）。
#[test]
fn read_only_table_blocks_writes() {
    let config = WallConfig::builder().read_only_table("archive").build();
    for sql in [
        "UPDATE archive SET a = 1 WHERE id = 1",
        "DELETE FROM archive WHERE id = 1",
        "INSERT INTO archive VALUES (1)",
        "TRUNCATE TABLE archive",
    ] {
        let blocked = violations_of(config.clone(), sql);
        assert!(
            blocked.contains(&WallViolation::ReadOnlyTable("archive".to_owned())),
            "sql={sql} violations={blocked:?}"
        );
    }
    // SELECT 只读表不触发（Java checkReadOnly 只作用于写入目标与 INTO）。
    let read = violations_of(config, "SELECT * FROM archive");
    assert!(
        !read.contains(&WallViolation::ReadOnlyTable("archive".to_owned())),
        "violations={read:?}"
    );
}

// ── WallConfigBuilder 新入口 ─────────────────────────────────

#[test]
fn builder_new_setters_roundtrip() {
    let config = WallConfig::builder()
        .use_allow(false)
        .show_allow(false)
        .describe_allow(false)
        .call_allow(false)
        .intersect_allow(false)
        .condition_and_alway_true_allow(false)
        .condition_double_const_allow(true)
        .case_condition_const_allow(true)
        .const_arithmetic_allow(false)
        .condition_op_bitwise_allow(false)
        .deny_variant("v")
        .read_only_table("archive")
        .build();
    assert!(!config.use_allow);
    assert!(!config.show_allow);
    assert!(!config.describe_allow);
    assert!(!config.call_allow);
    assert!(!config.intersect_allow);
    assert!(!config.condition_and_alway_true_allow);
    assert!(config.condition_double_const_allow);
    assert!(config.case_condition_const_allow);
    assert!(!config.const_arithmetic_allow);
    assert!(!config.condition_op_bitwise_allow);
    assert_eq!(config.deny_variants, vec!["v".to_owned()]);
    assert_eq!(config.read_only_tables, vec!["archive".to_owned()]);
}

// ── Display 可观察断言（测试技能：Display/Debug 独立验证）──────

/// 每个 variant 的 Display 输出独立断言（对应 Java 违规消息语义）。
#[test]
fn violation_display_all_variants() {
    let cases: Vec<(WallViolation, &str)> = vec![
        (
            WallViolation::OperationNotAllowed("SELECT".to_owned()),
            "SELECT not allowed",
        ),
        (
            WallViolation::MultiStatementNotAllowed,
            "multi-statement not allowed",
        ),
        (
            WallViolation::DropTableNotAllowed("t".to_owned()),
            "DROP TABLE not allowed: t",
        ),
        (WallViolation::TruncateNotAllowed, "TRUNCATE not allowed"),
        (
            WallViolation::DeleteWithoutWhere,
            "DELETE without WHERE not allowed",
        ),
        (
            WallViolation::UpdateWithoutWhere,
            "UPDATE without WHERE not allowed",
        ),
        (
            WallViolation::SelectAllColumnNotAllowed,
            "SELECT * not allowed",
        ),
        (
            WallViolation::AlwaysTrueCondition("part".to_owned()),
            "always true part condition not allowed",
        ),
        (
            WallViolation::AlwaysFalseCondition("part".to_owned()),
            "always false part condition not allowed",
        ),
        (
            WallViolation::DoubleConstCondition,
            "double const condition not allowed",
        ),
        (WallViolation::XorNotAllowed, "xor operator not allowed"),
        (
            WallViolation::BitwiseNotAllowed,
            "bitwise operator not allowed",
        ),
        (
            WallViolation::ConstArithmeticNotAllowed,
            "const arithmetic not allowed",
        ),
        (WallViolation::SameConstLike, "same const like not allowed"),
        (
            WallViolation::ConstCaseCondition,
            "const case condition not allowed",
        ),
        (
            WallViolation::MustParameterized,
            "sql must be parameterized",
        ),
        (WallViolation::UpdateCheckFailed, "update check failed."),
        (WallViolation::LimitZeroNotAllowed, "LIMIT 0 not allowed"),
        (
            WallViolation::DeniedTable("t".to_owned()),
            "denied table: t",
        ),
        (
            WallViolation::DeniedSchema("s".to_owned()),
            "denied schema: s",
        ),
        (
            WallViolation::DeniedFunction("f".to_owned()),
            "denied function: f",
        ),
        (
            WallViolation::DeniedVariant("@@v".to_owned()),
            "denied variant: @@v",
        ),
        (
            WallViolation::DeniedObject("o".to_owned()),
            "denied object: o",
        ),
        (
            WallViolation::ReadOnlyTable("t".to_owned()),
            "read only table: t",
        ),
        (
            WallViolation::SelectIntoOutfileNotAllowed,
            "select into outfile not allowed",
        ),
        (
            WallViolation::SyntaxError("bad".to_owned()),
            "syntax error: bad",
        ),
    ];
    for (violation, expected) in cases {
        assert_eq!(violation.to_string(), expected, "{violation:?}");
    }
}

// ── 既有字段路径的补充覆盖 ─────────────────────────────────────

/// `limitZeroAllow=false`（默认）→ LIMIT 0 `拒绝（ErrorCode.LIMIT_ZERO` 2200）。
#[test]
fn limit_zero_denied_by_default() {
    let violations = violations_of(WallConfig::default(), "SELECT * FROM t LIMIT 0");
    assert!(
        violations.contains(&WallViolation::LimitZeroNotAllowed),
        "{violations:?}"
    );
}

/// `commentAllow=false`（默认）→ 注释直接拒绝（1104）。
#[test]
fn comment_denied_by_default() {
    let violations = violations_of(WallConfig::default(), "SELECT 1 -- c");
    assert_eq!(
        violations,
        vec![WallViolation::OperationNotAllowed("COMMENT".to_owned())]
    );
}

/// `multiStatementAllow=false`（默认）→ 多语句拒绝（2201）。
#[test]
fn multi_statement_denied_by_default() {
    let violations = violations_of(WallConfig::default(), "SELECT 1; SELECT 2");
    assert!(
        violations.contains(&WallViolation::MultiStatementNotAllowed),
        "{violations:?}"
    );
}

/// `noneBaseStatementAllow=false`（默认）→ 未枚举语句类型拒绝（1999）。
#[test]
fn none_base_statement_denied_by_default() {
    for sql in [
        "ANALYZE TABLE t",
        "MERGE INTO a USING b ON a.id = b.id WHEN MATCHED THEN UPDATE SET a.x = b.x",
    ] {
        let violations = violations_of(WallConfig::default(), sql);
        assert!(
            violations.iter().any(|v| matches!(
                v,
                WallViolation::OperationNotAllowed(name) if name == "ANALYZE" || name == "MERGE"
            )),
            "sql={sql} violations={violations:?}"
        );
    }
}

/// `selectWhereAlwayTrueCheck=true`（默认）→ DELETE/UPDATE 整体恒真拒绝。
#[test]
fn delete_and_update_whole_alway_true_denied() {
    let delete = violations_of(WallConfig::default(), "DELETE FROM t WHERE 1 = 1");
    assert!(
        delete.contains(&WallViolation::AlwaysTrueCondition(
            "DELETE WHERE".to_owned()
        )),
        "{delete:?}"
    );
    let update = violations_of(WallConfig::default(), "UPDATE t SET a = 1 WHERE 1 = 1");
    assert!(
        update.contains(&WallViolation::AlwaysTrueCondition(
            "UPDATE WHERE".to_owned()
        )),
        "{update:?}"
    );
}

/// `mustParameterized=true` → 常量字面量拒绝（2200）。
#[test]
fn must_parameterized_denies_literals() {
    let config = WallConfig::builder().must_parameterized(true).build();
    let violations = violations_of(config, "SELECT * FROM t WHERE a = 1");
    assert!(
        violations.contains(&WallViolation::MustParameterized),
        "{violations:?}"
    );
}

/// `selectAllColumnAllow=false` → SELECT * 拒绝。
#[test]
fn select_all_column_denied_when_disabled() {
    let config = WallConfig::builder().select_all_column_allow(false).build();
    let violations = violations_of(config, "SELECT * FROM t");
    assert!(
        violations.contains(&WallViolation::SelectAllColumnNotAllowed),
        "{violations:?}"
    );
}

/// 常量求值覆盖 `NotEq` 与 Nested 路径（Java `getValue` 常量折叠语义）。
#[test]
fn const_value_covers_not_equal_and_nested() {
    let not_equal = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE 1 = 1 AND 2 <> 3",
    );
    assert!(
        not_equal.contains(&WallViolation::DoubleConstCondition),
        "{not_equal:?}"
    );
    let nested = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE ((1 = 1)) AND 2 = 2",
    );
    assert!(
        nested.contains(&WallViolation::DoubleConstCondition),
        "{nested:?}"
    );
}

// ── WallContext 警告计数与 update-check 路径 ─────────────────

use druid::core::Value;
use druid::sql::{WallContext, WallUpdateCheckHandler};
use std::sync::Arc;

struct RecordingHandler {
    allowed: bool,
}

impl WallUpdateCheckHandler for RecordingHandler {
    fn check(
        &self,
        _table: &str,
        _column: &str,
        _set_value: &Value,
        _filter_values: &[Value],
    ) -> bool {
        self.allowed
    }
}

#[test]
fn wall_context_counts_comment_and_none_condition_warnings() {
    WallContext::clear_context();
    let context = WallContext::create_if_not_exists(DbType::MySql);
    WallContext::set_context(Some(context.clone()));
    let wall = wall_with(WallConfig::builder().comment_allow(true).build());
    // 注释统计：非关键字后跟注释 → comment_count 增加。
    wall.check("SELECT 1 /* c */").unwrap();
    assert!(
        context.lock().comment_count() >= 1,
        "comment_count={}",
        context.lock().comment_count()
    );
    // UPDATE 无 WHERE → update_none_condition_warnings。
    wall.check("UPDATE t SET a = 1").unwrap();
    assert!(context.lock().update_none_condition_warnings() >= 1);
    // DELETE 无 WHERE 且无 JOIN → delete_none_condition_warnings。
    wall.check("DELETE FROM t").unwrap();
    assert!(context.lock().delete_none_condition_warnings() >= 1);
    // LIKE 数字 → like_number_warnings（Java check 仅计数不违规）。
    wall.check("SELECT * FROM t WHERE a LIKE 123").unwrap();
    assert!(context.lock().like_number_warnings() >= 1);
    WallContext::clear_context();
}

#[test]
fn update_check_handler_rejects_literal_overwrite() {
    let config = WallConfig::builder()
        .update_check_column("t.status")
        .update_check_handler(Arc::new(RecordingHandler { allowed: false }))
        .build();
    let wall = wall_with(config);
    let violations = wall
        .check("UPDATE t SET status = 'x' WHERE id = 1")
        .unwrap_err();
    assert!(
        violations.contains(&WallViolation::UpdateCheckFailed),
        "{violations:?}"
    );
}

#[test]
fn update_check_placeholder_defers_item() {
    let config = WallConfig::builder()
        .update_check_column("t.status")
        .update_check_handler(Arc::new(RecordingHandler { allowed: true }))
        .build();
    let wall = wall_with(config);
    // 占位符路径：无字面量可判 → 进入延迟检查项而不报错。
    wall.check("UPDATE t SET status = ? WHERE id = ?").unwrap();
}

#[test]
fn update_check_handler_allows_when_permitted() {
    let config = WallConfig::builder()
        .update_check_column("t.status")
        .update_check_handler(Arc::new(RecordingHandler { allowed: true }))
        .build();
    let wall = wall_with(config);
    wall.check("UPDATE t SET status = 'ok' WHERE id = 1")
        .unwrap();
}

// ── 边界路径补充（SELECT INTO / 双引号 LIKE / TRUE / hint / 引号转义）──

/// `selectIntoAllow` 关闭时 `SELECT a INTO b` `拒绝（ErrorCode.SELECT_INTO_NOT_ALLOW` 1003）。
#[test]
fn select_into_denied_when_disabled() {
    let config = WallConfig::builder().select_into_allow(false).build();
    let violations = violations_of(config, "SELECT a INTO b FROM t");
    assert!(
        violations.contains(&WallViolation::OperationNotAllowed(
            "SELECT INTO".to_owned()
        )),
        "{violations:?}"
    );
    // 默认（true）放行。
    assert!(violations_of(WallConfig::default(), "SELECT a INTO b FROM t").is_empty());
}

/// `startTransactionAllow=false` → START TRANSACTION 拒绝（ErrorCode 1303）。
#[test]
fn start_transaction_denied_when_disabled() {
    let config = WallConfig::builder().start_transaction_allow(false).build();
    let violations = violations_of(config, "START TRANSACTION");
    assert_eq!(
        violations,
        vec![WallViolation::OperationNotAllowed(
            "START TRANSACTION".to_owned()
        )]
    );
}

/// 双引号字符串常量 LIKE 也命中 same-const-like。
#[test]
fn same_const_like_double_quoted() {
    let violations = violations_of(
        WallConfig::default(),
        "SELECT * FROM t WHERE \"a\" LIKE \"a\"",
    );
    assert!(
        violations.contains(&WallViolation::SameConstLike),
        "{violations:?}"
    );
}

/// 整体 WHERE 为字面 TRUE → 恒真拒绝（Java `isSimpleConstExpr`）。
#[test]
fn where_literal_true_is_always_true() {
    let violations = violations_of(WallConfig::default(), "UPDATE t SET a = 1 WHERE TRUE");
    assert!(
        violations.contains(&WallViolation::AlwaysTrueCondition(
            "UPDATE WHERE".to_owned()
        )),
        "{violations:?}"
    );
}

/// `hintAllow=true`（默认）放行 `/*+ ... */` 优化器 hint；关闭后视为注释拒绝。
#[test]
fn hint_comment_gated_by_hint_allow() {
    assert!(violations_of(WallConfig::default(), "SELECT 1 /*+ hint */").is_empty());
    let blocked = violations_of(
        WallConfig::builder().hint_allow(false).build(),
        "SELECT 1 /*+ hint */",
    );
    assert_eq!(
        blocked,
        vec![WallViolation::OperationNotAllowed("COMMENT".to_owned())]
    );
}

/// `#` 注释在 `commentAllow=false` 下拒绝。
#[test]
fn hash_comment_denied_by_default() {
    let violations = violations_of(WallConfig::default(), "SELECT 1 # c");
    assert_eq!(
        violations,
        vec![WallViolation::OperationNotAllowed("COMMENT".to_owned())]
    );
}

/// 字符串字面量内的 `--` 不是注释（引号内跳过 + 双写引号转义）。
#[test]
fn comment_marker_inside_string_literal_not_detected() {
    assert!(violations_of(WallConfig::default(), "SELECT 'a''b -- not comment'").is_empty());
}

/// update-check 过滤条件覆盖 Eq 两侧字面量、IN 列表与复合列名。
#[test]
fn update_check_covers_eq_in_list_and_compound_columns() {
    use druid::core::Value;
    use druid::sql::WallUpdateCheckHandler;

    struct DenyAll;
    impl WallUpdateCheckHandler for DenyAll {
        fn check(&self, _: &str, _: &str, _: &Value, _: &[Value]) -> bool {
            false
        }
    }
    let config = WallConfig::builder()
        .update_check_column("t.status")
        .update_check_handler(Arc::new(DenyAll))
        .build();
    let wall = wall_with(config);
    // 字面量右侧 Eq。
    assert!(wall
        .check("UPDATE t SET status = 'x' WHERE t.status = 'old'")
        .is_err());
    // IN 列表。
    assert!(wall
        .check("UPDATE t SET status = 'x' WHERE t.status IN (1, 2, 3)")
        .is_err());
    // 复合列名（t.status）赋值目标。
    assert!(wall
        .check("UPDATE t SET t.status = 'x' WHERE t.status = 'old'")
        .is_err());
}
