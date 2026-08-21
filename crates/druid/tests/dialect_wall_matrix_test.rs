#![allow(clippy::match_same_arms)]
#![allow(clippy::type_complexity)]
//! Dialect Wall Matrix Differential Tests — Step 9-10
//!
//! Validates Wall visitor rule consistency across 7 SQL dialects:
//! `MySQL`, `PostgreSQL`, Oracle, `SQLServer`, DB2, `SQLite`, `ClickHouse`.
//!
//! Matrix: 9 SQL categories x 7 dialects = 63 base combinations,
//! plus `deny_tables` and `read_only_tables` tests per dialect.
//!
//! Key finding: `WallConfig::default()` sets `drop_table_allow`,
//! `truncate_allow`, and `alter_table_allow` to **true** (matching
//! Java `WallConfig` constructor). Tests verify both default-allow
//! and explicit-deny configurations.

use druid::sql::{
    CkWallProvider, Db2WallProvider, MySqlWallProvider, OracleWallProvider, PgWallProvider,
    SQLiteWallProvider, SqlServerWallProvider, WallCheckResult, WallConfig, WallProvider,
    WallViolation,
};

// ---------------------------------------------------------------------------
// Dialect enum + helper to create providers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    MySQL,
    PostgreSQL,
    Oracle,
    SqlServer,
    Db2,
    SQLite,
    ClickHouse,
}

const ALL_DIALECTS: &[Dialect] = &[
    Dialect::MySQL,
    Dialect::PostgreSQL,
    Dialect::Oracle,
    Dialect::SqlServer,
    Dialect::Db2,
    Dialect::SQLite,
    Dialect::ClickHouse,
];

fn dialect_name(d: Dialect) -> &'static str {
    match d {
        Dialect::MySQL => "MySQL",
        Dialect::PostgreSQL => "PostgreSQL",
        Dialect::Oracle => "Oracle",
        Dialect::SqlServer => "SqlServer",
        Dialect::Db2 => "DB2",
        Dialect::SQLite => "SQLite",
        Dialect::ClickHouse => "ClickHouse",
    }
}

fn make_provider(d: Dialect, config: WallConfig) -> WallProvider {
    match d {
        Dialect::MySQL => MySqlWallProvider::with_config(config).into_inner(),
        Dialect::PostgreSQL => PgWallProvider::with_config(config).into_inner(),
        Dialect::Oracle => OracleWallProvider::with_config(config).into_inner(),
        Dialect::SqlServer => SqlServerWallProvider::with_config(config).into_inner(),
        Dialect::Db2 => Db2WallProvider::with_config(config).into_inner(),
        Dialect::SQLite => SQLiteWallProvider::with_config(config).into_inner(),
        Dialect::ClickHouse => CkWallProvider::with_config(config).into_inner(),
    }
}

fn default_provider(d: Dialect) -> WallProvider {
    match d {
        Dialect::MySQL => MySqlWallProvider::new().into_inner(),
        Dialect::PostgreSQL => PgWallProvider::new().into_inner(),
        Dialect::Oracle => OracleWallProvider::new().into_inner(),
        Dialect::SqlServer => SqlServerWallProvider::new().into_inner(),
        Dialect::Db2 => Db2WallProvider::new().into_inner(),
        Dialect::SQLite => SQLiteWallProvider::new().into_inner(),
        Dialect::ClickHouse => CkWallProvider::new().into_inner(),
    }
}

fn check(p: &WallProvider, sql: &str) -> WallCheckResult {
    p.try_check(sql).expect("try_check should not panic")
}

fn has_violation(result: &WallCheckResult) -> bool {
    !result.violations().is_empty()
}

fn has_syntax_error(result: &WallCheckResult) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::SyntaxError(_)))
}

fn has_operation_not_allowed(result: &WallCheckResult, op: &str) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::OperationNotAllowed(s) if s.eq_ignore_ascii_case(op)))
}

fn has_drop_table_not_allowed(result: &WallCheckResult) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DropTableNotAllowed(_)))
}

fn has_truncate_not_allowed(result: &WallCheckResult) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::TruncateNotAllowed))
}

fn has_denied_table(result: &WallCheckResult) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_)))
}

fn has_read_only_table(result: &WallCheckResult) -> bool {
    result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::ReadOnlyTable(_)))
}

// ---------------------------------------------------------------------------
// SQL samples — chosen to be parseable by all 7 sqlparser dialects
// ---------------------------------------------------------------------------

/// Per-dialect SQL for basic CRUD. Oracle requires `FROM DUAL` for bare SELECT.
fn select_sql(d: Dialect) -> &'static str {
    match d {
        Dialect::Oracle => "SELECT id, name FROM orders",
        _ => "SELECT id, name FROM orders",
    }
}

fn insert_sql() -> &'static str {
    "INSERT INTO orders (id, name) VALUES (1, 'test')"
}

fn update_sql() -> &'static str {
    "UPDATE orders SET name = 'test' WHERE id = 1"
}

fn delete_sql() -> &'static str {
    "DELETE FROM orders WHERE id = 1"
}

fn drop_table_sql() -> &'static str {
    "DROP TABLE orders"
}

fn truncate_sql(d: Dialect) -> &'static str {
    // SQLite does not support TRUNCATE, so it will produce a SyntaxError.
    match d {
        Dialect::SQLite => "TRUNCATE orders",
        _ => "TRUNCATE orders",
    }
}

fn alter_table_sql() -> &'static str {
    "ALTER TABLE orders ADD COLUMN age INT"
}

fn grant_sql() -> &'static str {
    "GRANT SELECT ON orders TO user1"
}

fn revoke_sql() -> &'static str {
    "REVOKE SELECT ON orders FROM user1"
}

fn syntax_error_sql() -> &'static str {
    "SELCT * FORM orders"
}

fn deny_table_select_sql() -> &'static str {
    "SELECT id FROM secret_data"
}

fn read_only_insert_sql() -> &'static str {
    "INSERT INTO audit_log (id) VALUES (1)"
}

// ===========================================================================
// Test 1: CRUD operations — should pass on ALL dialects with default config
// ===========================================================================

#[test]
fn matrix_crud_select_allowed_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, select_sql(d));
        assert!(
            !has_violation(&result),
            "[{}] SELECT should be allowed, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_crud_insert_allowed_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, insert_sql());
        assert!(
            !has_violation(&result),
            "[{}] INSERT should be allowed, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_crud_update_allowed_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, update_sql());
        assert!(
            !has_violation(&result),
            "[{}] UPDATE should be allowed, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_crud_delete_allowed_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, delete_sql());
        assert!(
            !has_violation(&result),
            "[{}] DELETE should be allowed, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 2: DDL with default config — ALLOWED (WallConfig default drop_table_allow=true)
// ===========================================================================

#[test]
fn matrix_ddl_drop_default_allow_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, drop_table_sql());
        // WallConfig::default() has drop_table_allow=true, so DROP is allowed.
        assert!(
            !has_violation(&result),
            "[{}] DROP TABLE should be allowed with default config, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_ddl_truncate_default_allow_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, truncate_sql(d));
        // SQLite: sqlparser may not support TRUNCATE -> SyntaxError is acceptable.
        if d == Dialect::SQLite && has_syntax_error(&result) {
            continue;
        }
        // WallConfig::default() has truncate_allow=true, so TRUNCATE is allowed.
        assert!(
            !has_violation(&result),
            "[{}] TRUNCATE should be allowed with default config, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_ddl_alter_default_allow_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, alter_table_sql());
        // WallConfig::default() has alter_table_allow=true, so ALTER is allowed.
        assert!(
            !has_violation(&result),
            "[{}] ALTER TABLE should be allowed with default config, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 3: DDL with strict config (drop_table_allow=false, etc.) — should DENY
// ===========================================================================

#[test]
fn matrix_ddl_drop_strict_deny_all_dialects() {
    for &d in ALL_DIALECTS {
        let config = WallConfig::builder().drop_table_allow(false).build();
        let p = make_provider(d, config);
        let result = check(&p, drop_table_sql());
        assert!(
            has_drop_table_not_allowed(&result),
            "[{}] DROP TABLE should be denied when drop_table_allow=false, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_ddl_truncate_strict_deny_all_dialects() {
    for &d in ALL_DIALECTS {
        let config = WallConfig::builder().truncate_allow(false).build();
        let p = make_provider(d, config);
        let result = check(&p, truncate_sql(d));
        // SQLite: sqlparser may not support TRUNCATE -> SyntaxError is acceptable.
        if d == Dialect::SQLite && has_syntax_error(&result) {
            continue;
        }
        assert!(
            has_truncate_not_allowed(&result),
            "[{}] TRUNCATE should be denied when truncate_allow=false, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

#[test]
fn matrix_ddl_alter_strict_deny_all_dialects() {
    for &d in ALL_DIALECTS {
        let config = WallConfig::builder().alter_table_allow(false).build();
        let p = make_provider(d, config);
        let result = check(&p, alter_table_sql());
        assert!(
            has_operation_not_allowed(&result, "ALTER TABLE"),
            "[{}] ALTER TABLE should be denied when alter_table_allow=false, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 4: GRANT/REVOKE — should be denied (none_base_statement_allow=false)
// ===========================================================================

/// GRANT should be denied across all dialects. If sqlparser cannot parse it
/// for a given dialect, a `SyntaxError` is also acceptable (both prevent execution).
#[test]
fn matrix_grant_deny_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, grant_sql());
        assert!(
            has_violation(&result),
            "[{}] GRANT should be denied or syntax-error, got no violations",
            dialect_name(d),
        );
        // Accept either OperationNotAllowed or SyntaxError.
        let has_op = has_operation_not_allowed(&result, "GRANT");
        let has_syn = has_syntax_error(&result);
        assert!(
            has_op || has_syn,
            "[{}] GRANT should produce OperationNotAllowed(GRANT) or SyntaxError, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

/// REVOKE should be denied across all dialects.
#[test]
fn matrix_revoke_deny_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, revoke_sql());
        assert!(
            has_violation(&result),
            "[{}] REVOKE should be denied or syntax-error, got no violations",
            dialect_name(d),
        );
        let has_op = has_operation_not_allowed(&result, "REVOKE");
        let has_syn = has_syntax_error(&result);
        assert!(
            has_op || has_syn,
            "[{}] REVOKE should produce OperationNotAllowed(REVOKE) or SyntaxError, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 5: Syntax error — should report SyntaxError on ALL dialects
// ===========================================================================

#[test]
fn matrix_syntax_error_all_dialects() {
    for &d in ALL_DIALECTS {
        let p = default_provider(d);
        let result = check(&p, syntax_error_sql());
        assert!(
            has_syntax_error(&result),
            "[{}] malformed SQL should produce SyntaxError, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 6: deny_tables — SELECT from a denied table should produce DeniedTable
// ===========================================================================

#[test]
fn matrix_deny_table_select_all_dialects() {
    for &d in ALL_DIALECTS {
        let config = WallConfig::builder().deny_table("secret_data").build();
        let p = make_provider(d, config);
        let result = check(&p, deny_table_select_sql());
        assert!(
            has_denied_table(&result),
            "[{}] SELECT from denied table should produce DeniedTable, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 7: read_only_tables — INSERT into a read-only table should produce ReadOnlyTable
// ===========================================================================

#[test]
fn matrix_read_only_table_insert_all_dialects() {
    for &d in ALL_DIALECTS {
        let config = WallConfig::builder().read_only_table("audit_log").build();
        let p = make_provider(d, config);
        let result = check(&p, read_only_insert_sql());
        assert!(
            has_read_only_table(&result),
            "[{}] INSERT into read-only table should produce ReadOnlyTable, got: {:?}",
            dialect_name(d),
            result.violations()
        );
    }
}

// ===========================================================================
// Test 8: Comprehensive matrix — single test covering ALL 9 categories x 7 dialects
// ===========================================================================

/// Expected outcome for each SQL category under default config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Expected {
    Allow,
    Deny,
    DenyOrSyntax, // GRANT/REVOKE: OperationNotAllowed or SyntaxError
    SyntaxError,
    /// `SQLite` doesn't support TRUNCATE; `SyntaxError` is acceptable.
    DenyOrSkip,
}

struct MatrixEntry {
    dialect: Dialect,
    category: &'static str,
    sql_fn: fn(Dialect) -> &'static str,
    expected: Expected,
}

impl MatrixEntry {
    fn new(
        dialect: Dialect,
        category: &'static str,
        sql_fn: fn(Dialect) -> &'static str,
        expected: Expected,
    ) -> Self {
        Self {
            dialect,
            category,
            sql_fn,
            expected,
        }
    }
}

/// Build the full matrix of 9 categories x 7 dialects.
fn build_full_matrix() -> Vec<MatrixEntry> {
    let mut entries = Vec::with_capacity(63);

    // Define per-category SQL functions
    fn sql_select(d: Dialect) -> &'static str {
        select_sql(d)
    }
    fn sql_insert(_d: Dialect) -> &'static str {
        insert_sql()
    }
    fn sql_update(_d: Dialect) -> &'static str {
        update_sql()
    }
    fn sql_delete(_d: Dialect) -> &'static str {
        delete_sql()
    }
    fn sql_drop(_d: Dialect) -> &'static str {
        drop_table_sql()
    }
    fn sql_truncate(d: Dialect) -> &'static str {
        truncate_sql(d)
    }
    fn sql_alter(_d: Dialect) -> &'static str {
        alter_table_sql()
    }
    fn sql_grant(_d: Dialect) -> &'static str {
        grant_sql()
    }
    fn sql_revoke(_d: Dialect) -> &'static str {
        revoke_sql()
    }

    let categories: &[(&str, fn(Dialect) -> &'static str, Expected)] = &[
        ("SELECT", sql_select, Expected::Allow),
        ("INSERT", sql_insert, Expected::Allow),
        ("UPDATE", sql_update, Expected::Allow),
        ("DELETE", sql_delete, Expected::Allow),
        ("DROP TABLE", sql_drop, Expected::Allow), // default allow
        ("TRUNCATE", sql_truncate, Expected::DenyOrSkip), // default allow, but SQLite can't parse
        ("ALTER TABLE", sql_alter, Expected::Allow), // default allow
        ("GRANT", sql_grant, Expected::DenyOrSyntax),
        ("REVOKE", sql_revoke, Expected::DenyOrSyntax),
    ];

    for &(category, sql_fn, expected) in categories {
        for &dialect in ALL_DIALECTS {
            entries.push(MatrixEntry::new(dialect, category, sql_fn, expected));
        }
    }
    entries
}

#[test]
fn matrix_full_9x7_default_config() {
    let matrix = build_full_matrix();
    let mut total = 0_usize;
    let mut passed = 0_usize;
    let mut inconsistencies: Vec<String> = Vec::new();

    for entry in &matrix {
        total += 1;
        let p = default_provider(entry.dialect);
        let sql = (entry.sql_fn)(entry.dialect);
        let result = check(&p, sql);
        let violated = has_violation(&result);
        let syntax_err = has_syntax_error(&result);

        let ok = match entry.expected {
            Expected::Allow => !violated,
            Expected::Deny => violated && !syntax_err,
            Expected::DenyOrSyntax => violated, // either OperationNotAllowed or SyntaxError
            Expected::SyntaxError => syntax_err,
            Expected::DenyOrSkip => {
                // SQLite TRUNCATE: SyntaxError is acceptable; otherwise should be allowed
                if entry.dialect == Dialect::SQLite && syntax_err {
                    true
                } else {
                    !violated
                }
            }
        };

        if ok {
            passed += 1;
        } else {
            inconsistencies.push(format!(
                "[{} {}] expected={:?}, got violations={:?}",
                dialect_name(entry.dialect),
                entry.category,
                entry.expected,
                result.violations()
            ));
        }
    }

    assert!(
        inconsistencies.is_empty(),
        "Matrix test: {}/{} passed. Inconsistencies:\n{}",
        passed,
        total,
        inconsistencies.join("\n")
    );
}

// ===========================================================================
// Test 9: Strict config matrix — DROP/TRUNCATE/ALTER should all be denied
// ===========================================================================

#[test]
fn matrix_strict_ddl_3x7() {
    let strict_config = || {
        WallConfig::builder()
            .drop_table_allow(false)
            .truncate_allow(false)
            .alter_table_allow(false)
            .build()
    };

    let ddl_cases: &[(
        &str,
        fn(Dialect) -> &'static str,
        fn(&WallCheckResult) -> bool,
    )] = &[
        (
            "DROP TABLE",
            drop_table_sql_dialect,
            has_drop_table_not_allowed,
        ),
        ("TRUNCATE", truncate_sql, has_truncate_not_allowed),
        ("ALTER TABLE", alter_table_sql_dialect, |r| {
            has_operation_not_allowed(r, "ALTER TABLE")
        }),
    ];

    let mut inconsistencies: Vec<String> = Vec::new();
    let mut total = 0_usize;
    let mut passed = 0_usize;

    for &(category, sql_fn, check_fn) in ddl_cases {
        for &dialect in ALL_DIALECTS {
            total += 1;
            let p = make_provider(dialect, strict_config());
            let sql = sql_fn(dialect);
            let result = check(&p, sql);

            // SQLite TRUNCATE is a SyntaxError — acceptable as "denied"
            if category == "TRUNCATE" && dialect == Dialect::SQLite && has_syntax_error(&result) {
                passed += 1;
                continue;
            }

            if check_fn(&result) {
                passed += 1;
            } else {
                inconsistencies.push(format!(
                    "[{} {}] expected denied, got: {:?}",
                    dialect_name(dialect),
                    category,
                    result.violations()
                ));
            }
        }
    }

    assert!(
        inconsistencies.is_empty(),
        "Strict DDL matrix: {}/{} passed. Inconsistencies:\n{}",
        passed,
        total,
        inconsistencies.join("\n")
    );
}

fn drop_table_sql_dialect(_d: Dialect) -> &'static str {
    drop_table_sql()
}

fn alter_table_sql_dialect(_d: Dialect) -> &'static str {
    alter_table_sql()
}

// ===========================================================================
// Test 10: Cross-dialect consistency — same SQL, same config, same verdict
// ===========================================================================

/// Verify that SELECT/INSERT/UPDATE/DELETE produce the SAME verdict
/// (all-allow or all-deny) across all 7 dialects.
#[test]
fn matrix_cross_dialect_consistency_crud() {
    let test_cases: &[(&str, fn(Dialect) -> &'static str)] = &[
        ("SELECT", select_sql),
        ("INSERT", |_: Dialect| insert_sql()),
        ("UPDATE", |_: Dialect| update_sql()),
        ("DELETE", |_: Dialect| delete_sql()),
    ];

    for &(category, sql_fn) in test_cases {
        let results: Vec<(Dialect, bool)> = ALL_DIALECTS
            .iter()
            .map(|&d| {
                let p = default_provider(d);
                let r = check(&p, sql_fn(d));
                (d, has_violation(&r))
            })
            .collect();

        let any_denied = results.iter().any(|(_, v)| *v);
        let all_denied = results.iter().all(|(_, v)| *v);

        assert!(
            !any_denied,
            "[{category}] should be allowed on all dialects, but some denied: {:?}",
            results
                .iter()
                .filter(|(_, v)| *v)
                .map(|(d, _)| dialect_name(*d))
                .collect::<Vec<_>>()
        );
        // Consistency: all must agree
        assert!(
            !any_denied || all_denied,
            "[{category}] inconsistent across dialects: {:?}",
            results
                .iter()
                .map(|(d, v)| format!("{}={}", dialect_name(*d), v))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// Verify that DROP TABLE with strict config produces the SAME deny verdict
/// across all 7 dialects.
#[test]
fn matrix_cross_dialect_consistency_drop_strict() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let results: Vec<(Dialect, bool)> = ALL_DIALECTS
        .iter()
        .map(|&d| {
            let p = make_provider(d, config.clone());
            let r = check(&p, drop_table_sql());
            (d, has_drop_table_not_allowed(&r))
        })
        .collect();

    let all_denied = results.iter().all(|(_, v)| *v);
    assert!(
        all_denied,
        "DROP TABLE (strict) should be denied on all dialects, got: {:?}",
        results
            .iter()
            .map(|(d, v)| format!("{}={}", dialect_name(*d), v))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

/// Verify `deny_tables` produces consistent `DeniedTable` across all 7 dialects.
#[test]
fn matrix_cross_dialect_consistency_deny_table() {
    let config = WallConfig::builder().deny_table("secret_data").build();
    let results: Vec<(Dialect, bool)> = ALL_DIALECTS
        .iter()
        .map(|&d| {
            let p = make_provider(d, config.clone());
            let r = check(&p, deny_table_select_sql());
            (d, has_denied_table(&r))
        })
        .collect();

    let all_denied = results.iter().all(|(_, v)| *v);
    assert!(
        all_denied,
        "deny_tables should produce DeniedTable on all dialects, got: {:?}",
        results
            .iter()
            .map(|(d, v)| format!("{}={}", dialect_name(*d), v))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

/// Verify `read_only_tables` produces consistent `ReadOnlyTable` across all 7 dialects.
#[test]
fn matrix_cross_dialect_consistency_read_only() {
    let config = WallConfig::builder().read_only_table("audit_log").build();
    let results: Vec<(Dialect, bool)> = ALL_DIALECTS
        .iter()
        .map(|&d| {
            let p = make_provider(d, config.clone());
            let r = check(&p, read_only_insert_sql());
            (d, has_read_only_table(&r))
        })
        .collect();

    let all_denied = results.iter().all(|(_, v)| *v);
    assert!(
        all_denied,
        "read_only_tables should produce ReadOnlyTable on all dialects, got: {:?}",
        results
            .iter()
            .map(|(d, v)| format!("{}={}", dialect_name(*d), v))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
