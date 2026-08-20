//! Batch 2 coverage tests for merge.rs (SqlMerger + parameterize).
//!
//! Targets uncovered branches:
//! - parameterize: hex/binary numbers, scientific notation, negative numbers,
//!   bracket identifiers, block comments, hash comments, escaped quotes, N'...' strings,
//!   underscored numbers, dot-prefixed decimals
//! - SqlMerger: capacity eviction (skip_sql_count), set_max_sql_size shrink,
//!   reset retains active/evicts empty, active_stat_for_sql, take_skip_sql_count,
//!   record_with_merge_stat, record (merge_sql=false path via sql_key)

extern crate druid_core as druid;
use druid::stats::{fingerprint, parameterize, SqlMerger};
use std::time::Duration;

// ===========================================================================
// 1. parameterize edge cases
// ===========================================================================

#[test]
fn parameterize_hex_number() {
    let result = parameterize("SELECT 0xFF FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_hex_number_uppercase() {
    let result = parameterize("SELECT 0XAB FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_binary_number() {
    let result = parameterize("SELECT 0b1010 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_binary_number_uppercase() {
    let result = parameterize("SELECT 0B1010 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_hex_with_underscores() {
    let result = parameterize("SELECT 0xFF_FF FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_scientific_notation() {
    let result = parameterize("SELECT 1e10 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_scientific_notation_uppercase() {
    let result = parameterize("SELECT 1E10 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_scientific_notation_with_sign() {
    let result = parameterize("SELECT 1e+10 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_scientific_notation_negative_exp() {
    let result = parameterize("SELECT 1e-10 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_scientific_notation_no_digits_after_e() {
    // When e/E is not followed by digits, it's not consumed as part of the number
    let result = parameterize("SELECT 1e FROM t");
    // '1' is consumed as number, 'e' remains
    assert_eq!(result.template, "SELECT ?e FROM t");
}

#[test]
fn parameterize_dot_prefixed_decimal() {
    let result = parameterize("SELECT .5 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_number_with_underscore() {
    let result = parameterize("SELECT 1_000 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_decimal_with_underscore() {
    let result = parameterize("SELECT 1_000.5_0 FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_negative_number() {
    // Negative sign '-' followed by digit at token start
    let result = parameterize("SELECT -1 FROM t");
    // '-' is a separate token, '1' at token start → number
    assert_eq!(result.template, "SELECT -? FROM t");
}

#[test]
fn parameterize_bracket_identifier() {
    let result = parameterize("SELECT [column name] FROM t");
    assert_eq!(result.template, "SELECT [column name] FROM t");
}

#[test]
fn parameterize_bracket_identifier_with_escape() {
    let result = parameterize("SELECT [col]]umn] FROM t");
    assert_eq!(result.template, "SELECT [col]]umn] FROM t");
}

#[test]
fn parameterize_block_comment() {
    let result = parameterize("SELECT /* comment */ 1 FROM t");
    assert_eq!(result.template, "SELECT /* comment */ ? FROM t");
}

#[test]
fn parameterize_line_comment_dash_dash() {
    let result = parameterize("SELECT 1 -- comment\nFROM t");
    assert_eq!(result.template, "SELECT ? -- comment\nFROM t");
}

#[test]
fn parameterize_hash_comment() {
    let result = parameterize("SELECT 1 # comment\nFROM t");
    assert_eq!(result.template, "SELECT ? # comment\nFROM t");
}

#[test]
fn parameterize_n_string_prefix() {
    // N'...' prefix — the N is popped and the string is replaced with ?
    let result = parameterize("SELECT N'hello' FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_x_string_prefix() {
    let result = parameterize("SELECT x'FF' FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_b_string_prefix() {
    let result = parameterize("SELECT b'1010' FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_escaped_quote_in_string() {
    let result = parameterize("SELECT 'it''s' FROM t");
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_backslash_escaped_quote() {
    let result = parameterize(r#"SELECT 'it\'s' FROM t"#);
    assert_eq!(result.template, "SELECT ? FROM t");
}

#[test]
fn parameterize_double_quoted_identifier() {
    let result = parameterize(r#"SELECT "column" FROM t"#);
    assert_eq!(result.template, r#"SELECT "column" FROM t"#);
}

#[test]
fn parameterize_backtick_identifier() {
    let result = parameterize("SELECT `column` FROM t");
    assert_eq!(result.template, "SELECT `column` FROM t");
}

#[test]
fn parameterize_unterminated_string() {
    let result = parameterize("SELECT 'unterminated FROM t");
    // Unterminated string consumes to end of input
    assert_eq!(result.template, "SELECT ?");
}

#[test]
fn parameterize_unterminated_block_comment() {
    let result = parameterize("SELECT /* unterminated 1 FROM t");
    // The entire remaining is consumed as comment
    assert!(result.template.contains("/*"));
}

#[test]
fn parameterize_number_not_at_token_start() {
    // e.g., 'table1' — '1' is not at token start
    let result = parameterize("SELECT * FROM table1");
    assert_eq!(result.template, "SELECT * FROM table1");
}

#[test]
fn parameterize_placeholder_preserved() {
    let result = parameterize("SELECT * FROM t WHERE id = ?");
    assert_eq!(result.template, "SELECT * FROM t WHERE id = ?");
}

#[test]
fn parameterize_fingerprint_consistency() {
    let p1 = parameterize("SELECT * FROM t WHERE id = 1");
    let p2 = parameterize("SELECT * FROM t WHERE id = 2");
    assert_eq!(
        p1.fingerprint, p2.fingerprint,
        "Same template should produce same fingerprint"
    );
    assert_eq!(p1.template, p2.template);
}

#[test]
fn fingerprint_function() {
    let f1 = fingerprint("SELECT ?");
    let f2 = fingerprint("SELECT ?");
    let f3 = fingerprint("INSERT ?");
    assert_eq!(f1, f2);
    assert_ne!(f1, f3);
}

// ===========================================================================
// 2. SqlMerger capacity eviction
// ===========================================================================

#[test]
fn sql_merger_eviction_increments_skip_count() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(2);
    // Use merge_sql=false to get distinct fingerprints
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO b VALUES (2)",
        Duration::from_millis(1),
        true,
        false,
    );
    // Third record triggers eviction of the oldest
    merger.record_with_merge(
        "INSERT INTO c VALUES (3)",
        Duration::from_millis(1),
        true,
        false,
    );
    assert_eq!(merger.skip_sql_count(), 1);
    assert_eq!(merger.len(), 2);
}

#[test]
fn sql_merger_eviction_of_unexecuted_stat_does_not_increment_skip() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(1);
    // prepare without recording execution (merge_sql=false for distinct key)
    merger.prepare("INSERT INTO a VALUES (1)", false);
    merger.record_with_merge(
        "INSERT INTO b VALUES (2)",
        Duration::from_millis(1),
        true,
        false,
    );
    // The first one had execute_count=0, running_count=0, so skip_sql_count stays 0
    assert_eq!(merger.skip_sql_count(), 0);
}

#[test]
fn sql_merger_set_max_sql_size_shrink() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(10);
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO b VALUES (2)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO c VALUES (3)",
        Duration::from_millis(1),
        true,
        false,
    );
    assert_eq!(merger.len(), 3);
    // Shrink to 1: Java removes old - new = 10 - 1 = 9 entries
    merger.set_max_sql_size(1);
    assert_eq!(merger.len(), 0);
}

#[test]
fn sql_merger_set_max_sql_size_shrink_partial() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(10);
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO b VALUES (2)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO c VALUES (3)",
        Duration::from_millis(1),
        true,
        false,
    );
    // old=10, new=2, remove_count=10-2=8, but only 3 exist
    merger.set_max_sql_size(2);
    assert_eq!(merger.len(), 0);
}

#[test]
fn sql_merger_set_max_sql_size_grow_noop() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(2);
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.set_max_sql_size(10);
    assert_eq!(merger.len(), 1);
}

#[test]
fn sql_merger_reset_retains_active_evicts_empty() {
    let merger = SqlMerger::new();
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    // prepare only (no execute), distinct key
    merger.prepare("INSERT INTO b VALUES (2)", false);
    assert_eq!(merger.len(), 2);
    merger.reset();
    // a had execute_count > 0, so it's retained (but reset)
    // b had execute_count == 0 and running_count == 0, so it's evicted
    assert_eq!(merger.len(), 1);
}

#[test]
fn sql_merger_reset_clears_skip_count() {
    let merger = SqlMerger::new();
    merger.set_max_sql_size(1);
    merger.record_with_merge(
        "INSERT INTO a VALUES (1)",
        Duration::from_millis(1),
        true,
        false,
    );
    merger.record_with_merge(
        "INSERT INTO b VALUES (2)",
        Duration::from_millis(1),
        true,
        false,
    );
    assert!(merger.skip_sql_count() > 0);
    merger.reset();
    assert_eq!(merger.skip_sql_count(), 0);
}

#[test]
fn sql_merger_active_stat_for_sql() {
    let merger = SqlMerger::new();
    merger.record(
        "SELECT * FROM t WHERE id = 1",
        Duration::from_millis(1),
        true,
    );
    let stat = merger.active_stat_for_sql("SELECT * FROM t WHERE id = 1");
    assert!(stat.is_some());
}

#[test]
fn sql_merger_active_stat_for_unknown_sql() {
    let merger = SqlMerger::new();
    assert!(merger.active_stat_for_sql("UNKNOWN").is_none());
}

#[test]
fn sql_merger_record_with_merge_false() {
    let merger = SqlMerger::new();
    merger.record_with_merge(
        "SELECT * FROM t WHERE id = 1",
        Duration::from_millis(1),
        true,
        false,
    );
    // With merge_sql=false, the raw SQL is used as key
    let stats = merger.all_stats();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].sql, "SELECT * FROM t WHERE id = 1");
}

#[test]
fn sql_merger_record_with_merge_true() {
    let merger = SqlMerger::new();
    merger.record_with_merge(
        "SELECT * FROM t WHERE id = 1",
        Duration::from_millis(1),
        true,
        true,
    );
    merger.record_with_merge(
        "SELECT * FROM t WHERE id = 2",
        Duration::from_millis(1),
        true,
        true,
    );
    // With merge_sql=true, both should map to same template
    let stats = merger.all_stats();
    assert_eq!(stats.len(), 1);
}

#[test]
fn sql_merger_record_with_merge_stat_returns_stat() {
    let merger = SqlMerger::new();
    let stat = merger.record_with_merge_stat("SELECT 1", Duration::from_millis(1), true, true);
    assert!(stat.execute_count() > 0);
}

#[test]
fn sql_merger_is_empty() {
    let merger = SqlMerger::new();
    assert!(merger.is_empty());
    merger.record("SELECT 1", Duration::from_millis(1), true);
    assert!(!merger.is_empty());
}

#[test]
fn sql_merger_default() {
    let merger = SqlMerger::default();
    assert!(merger.is_empty());
    assert_eq!(merger.max_sql_size(), 1000);
}

#[test]
fn sql_merger_get_stat_unknown() {
    let merger = SqlMerger::new();
    assert!(merger.get_stat(999).is_none());
}

// ===========================================================================
// 3. parameterize empty and edge inputs
// ===========================================================================

#[test]
fn parameterize_empty_string() {
    let result = parameterize("");
    assert_eq!(result.template, "");
}

#[test]
fn parameterize_only_whitespace() {
    let result = parameterize("   ");
    assert_eq!(result.template, "   ");
}

#[test]
fn parameterize_only_comment() {
    let result = parameterize("-- just a comment");
    assert_eq!(result.template, "-- just a comment");
}

// ===========================================================================
// 4. bracket identifier edge cases
// ===========================================================================

#[test]
fn parameterize_unterminated_bracket_identifier() {
    let result = parameterize("SELECT [unterminated FROM t");
    // Unterminated bracket consumes to end
    assert!(result.template.contains("[unterminated FROM t"));
}

// ===========================================================================
// 5. dot-prefixed decimal not at token start
// ===========================================================================

#[test]
fn parameterize_dot_not_at_token_start() {
    let result = parameterize("SELECT t.col FROM t");
    // '.' is not at token start (preceded by 't'), so not consumed as number
    assert_eq!(result.template, "SELECT t.col FROM t");
}
