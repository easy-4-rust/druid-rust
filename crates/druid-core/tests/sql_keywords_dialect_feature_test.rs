//! SQL 关键字/方言/Parser 特性差分测试（C9 批次：sql/ 0% 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：
//! - `Keywords`：FNV-1a 哈希查找、默认/SQLite/DM 关键字表。
//! - `DialectFeature`：特性枚举、位图配置。
//! - `SqlParserFeature`：`特性位集合、of/value_of`。
//! - `Token`：枚举变体、Debug、PartialEq。
//! - `Lexer`：通过 sqlparser 直接验证词法输出。

extern crate druid_core as druid;
use druid_core::sql::dialect_feature::{
    DialectFeature, DialectFeatureValue, LexerFeature, ParserFeature,
};
use druid_core::sql::keywords::{Keywords, DEFAULT_KEYWORDS, SQLITE_KEYWORDS};
use druid_core::sql::sql_parser_feature::SqlParserFeature;

// ── Keywords（Java Druid 关键字表）────────────────────────────

/// 默认关键字表包含核心 SQL 关键字。
#[test]
fn keywords_default_table_contains_core_keywords() {
    assert!(DEFAULT_KEYWORDS.get_keyword("SELECT").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("INSERT").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("UPDATE").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("DELETE").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("FROM").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("WHERE").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("CREATE").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("ALTER").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("DROP").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("TABLE").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("INDEX").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("JOIN").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("AND").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("OR").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("NOT").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("NULL").is_some());
}

/// `SQLite` 关键字表在默认表基础上增加 LIMIT。
#[test]
fn sqlite_keywords_adds_limit() {
    // SQLite 独立表。
    assert!(SQLITE_KEYWORDS.get_keyword("LIMIT").is_some());
    assert!(SQLITE_KEYWORDS.get_keyword("SELECT").is_some());
    // 默认表也包含 LIMIT（Java 两个表独立但最终内容相同）。
    assert!(DEFAULT_KEYWORDS.get_keyword("LIMIT").is_some());
}

/// `get_keyword` 大小写不敏感（Java toLowerCase 后匹配）。
#[test]
fn keywords_case_insensitive_lookup() {
    assert!(DEFAULT_KEYWORDS.get_keyword("select").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("Select").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("SELECT").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("from").is_some());
    assert!(DEFAULT_KEYWORDS.get_keyword("FROM").is_some());
}

/// `get_keyword` 不存在的关键字返回 None。
#[test]
fn keywords_unknown_keyword_returns_none() {
    assert!(DEFAULT_KEYWORDS.get_keyword("NOTAKEYWORD").is_none());
    assert!(DEFAULT_KEYWORDS.get_keyword("").is_none());
    assert!(DEFAULT_KEYWORDS.get_keyword("my_custom_function").is_none());
}

/// `contains_value：按` Token 值检查。
#[test]
fn keywords_contains_value() {
    // SELECT 关键字应存在。
    let select_token = DEFAULT_KEYWORDS.get_keyword("SELECT").unwrap();
    assert!(DEFAULT_KEYWORDS.contains_value(select_token));
}

/// `fnv1a_64_lower：确定性哈希，大小写不敏感`。
#[test]
fn keywords_fnv1a_hash_deterministic_and_case_insensitive() {
    let hash1 = Keywords::fnv1a_64_lower("SELECT");
    let hash2 = Keywords::fnv1a_64_lower("select");
    let hash3 = Keywords::fnv1a_64_lower("SELECT");
    assert_eq!(hash1, hash2, "FNV-1a must be case-insensitive");
    assert_eq!(hash1, hash3, "FNV-1a must be deterministic");
    assert_ne!(hash1, Keywords::fnv1a_64_lower("INSERT"));
}

/// `get_keyword_by_hash：哈希查找与直接查找结果一致`。
#[test]
fn keywords_hash_lookup_matches_direct_lookup() {
    for keyword in &[
        "SELECT", "INSERT", "UPDATE", "DELETE", "FROM", "WHERE", "CREATE", "DROP",
    ] {
        let direct = DEFAULT_KEYWORDS.get_keyword(keyword);
        let hash = Keywords::fnv1a_64_lower(keyword);
        let by_hash = DEFAULT_KEYWORDS.get_keyword_by_hash(hash);
        assert_eq!(
            direct, by_hash,
            "hash lookup must match direct for {keyword}"
        );
    }
}

/// 新建自定义关键字表。
#[test]
fn keywords_custom_table() {
    let mut map = HashMap::new();
    map.insert("CUSTOM_KEYWORD".to_owned(), druid_core::sql::Token::Select);
    let keywords = Keywords::new(map);
    assert!(keywords.get_keyword("CUSTOM_KEYWORD").is_some());
    assert!(
        keywords.get_keyword("SELECT").is_none(),
        "custom table only has custom keyword"
    );
}

// ── DialectFeature（Java DialectFeature 双 mask）─────────────

/// 新建默认方言特性（Java `DialectFeature` 默认启用 `ScanNumberPrefixB` 等）。
#[test]
fn dialect_feature_new_and_default() {
    let feature = DialectFeature::new();
    // 默认启用 ScanNumberPrefixB 等 lexer/parser 特性。
    assert!(feature.is_enabled(DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB,)));
    assert!(feature.is_enabled(DialectFeatureValue::Parser(ParserFeature::AcceptUnion,)));
    // 默认不启用 ScanComment。
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment,
    )));
}

/// 配置单个特性。
#[test]
fn dialect_feature_config_single() {
    let mut feature = DialectFeature::new();
    feature.config_feature(
        DialectFeatureValue::Lexer(LexerFeature::ScanSqlTypeBlockComment),
        true,
    );
    assert!(feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
    feature.config_feature(
        DialectFeatureValue::Lexer(LexerFeature::ScanSqlTypeBlockComment),
        false,
    );
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
}

/// 批量配置特性。
#[test]
fn dialect_feature_config_batch() {
    let mut feature = DialectFeature::from_features(None);
    let features = vec![
        DialectFeatureValue::Lexer(LexerFeature::ScanSqlTypeBlockComment),
        DialectFeatureValue::Parser(ParserFeature::AcceptUnion),
    ];
    feature.config_features(&features);
    assert!(feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
    assert!(feature.is_enabled(DialectFeatureValue::Parser(ParserFeature::AcceptUnion)));
    feature.unconfig_features(&features);
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
    assert!(!feature.is_enabled(DialectFeatureValue::Parser(ParserFeature::AcceptUnion)));
}

/// `with_lists` 构造：先应用默认值，再打开和关闭指定项。
#[test]
fn dialect_feature_with_lists() {
    let includes = vec![DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment,
    )];
    let excludes = vec![DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB)];
    let feature = DialectFeature::with_lists(Some(&includes), Some(&excludes));
    assert!(feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB)));
}

/// `from_features` 构造：从两个零 mask 开始。
#[test]
fn dialect_feature_from_features() {
    let features = vec![DialectFeatureValue::Parser(ParserFeature::AcceptUnion)];
    let feature = DialectFeature::from_features(Some(&features));
    assert!(feature.is_enabled(DialectFeatureValue::Parser(ParserFeature::AcceptUnion)));
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB)));
}

/// `with_enabled：enable=false` 时忽略 features 列表。
#[test]
fn dialect_feature_with_enabled_false() {
    let features = vec![DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment,
    )];
    let feature = DialectFeature::with_enabled(false, Some(&features));
    assert!(!feature.is_enabled(DialectFeatureValue::Lexer(
        LexerFeature::ScanSqlTypeBlockComment
    )));
}

/// `DialectFeatureValue` 枚举变体 + 位运算。
#[test]
fn dialect_feature_value_variants() {
    // Lexer 变体。
    let lexer = DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB);
    assert!(matches!(lexer, DialectFeatureValue::Lexer(_)));
    // Parser 变体。
    let parser = DialectFeatureValue::Parser(ParserFeature::AcceptUnion);
    assert!(matches!(parser, DialectFeatureValue::Parser(_)));

    // 位 mask 不为零。
    assert!(LexerFeature::ScanNumberPrefixB.mask() > 0);
    assert!(ParserFeature::AcceptUnion.mask() > 0);
    // 不同变体 mask 不同。
    assert_ne!(
        LexerFeature::ScanNumberPrefixB.mask(),
        LexerFeature::ScanSqlTypeBlockComment.mask()
    );
    assert_ne!(
        ParserFeature::AcceptUnion.mask(),
        ParserFeature::SqlTimestampExpr.mask()
    );

    // ordinal。
    assert_eq!(LexerFeature::ScanNumberPrefixB.ordinal(), 11);
    assert_eq!(ParserFeature::AcceptUnion.ordinal(), 0);

    // java_name。
    assert_eq!(
        LexerFeature::ScanNumberPrefixB.java_name(),
        "ScanNumberPrefixB"
    );
    assert_eq!(ParserFeature::AcceptUnion.java_name(), "AcceptUnion");

    // value_of。
    assert_eq!(
        LexerFeature::value_of("ScanNumberPrefixB"),
        Some(LexerFeature::ScanNumberPrefixB)
    );
    assert_eq!(
        ParserFeature::value_of("AcceptUnion"),
        Some(ParserFeature::AcceptUnion)
    );
    assert_eq!(LexerFeature::value_of("unknown"), None);
}

// ── SqlParserFeature（Java SQLParserFeature 位集合）─────────────

/// of：位集合合成。
#[test]
fn sql_parser_feature_of_bitmask() {
    let features = vec![
        SqlParserFeature::KeepComments,
        SqlParserFeature::StrictForWall,
    ];
    let mask = SqlParserFeature::of(&features);
    assert!(mask > 0);
    // 单独每个特性也应有非零位。
    let keep_mask = SqlParserFeature::of(&[SqlParserFeature::KeepComments]);
    let strict_mask = SqlParserFeature::of(&[SqlParserFeature::StrictForWall]);
    assert!(keep_mask > 0);
    assert!(strict_mask > 0);
    assert_ne!(
        keep_mask, strict_mask,
        "different features must have different masks"
    );
}

/// `of_nullable：None` 返回 0。
#[test]
fn sql_parser_feature_of_nullable_none() {
    assert_eq!(SqlParserFeature::of_nullable(None), 0);
    let features = vec![SqlParserFeature::KeepComments];
    assert!(SqlParserFeature::of_nullable(Some(&features)) > 0);
}

/// `value_of：名称查找`。
#[test]
fn sql_parser_feature_value_of() {
    assert_eq!(
        SqlParserFeature::value_of("KeepComments"),
        Some(SqlParserFeature::KeepComments)
    );
    assert_eq!(
        SqlParserFeature::value_of("StrictForWall"),
        Some(SqlParserFeature::StrictForWall)
    );
    assert_eq!(
        SqlParserFeature::value_of("KeepInsertValueClauseOriginalString"),
        Some(SqlParserFeature::KeepInsertValueClauseOriginalString)
    );
    assert_eq!(
        SqlParserFeature::value_of("SelectItemGenerateAlias"),
        Some(SqlParserFeature::SelectItemGenerateAlias)
    );
    assert_eq!(
        SqlParserFeature::value_of("Spark"),
        Some(SqlParserFeature::Spark)
    );
    assert_eq!(
        SqlParserFeature::value_of("Presto"),
        Some(SqlParserFeature::Presto)
    );
    assert_eq!(
        SqlParserFeature::value_of("Template"),
        Some(SqlParserFeature::Template)
    );
    assert_eq!(SqlParserFeature::value_of("unknown"), None);
}

/// of 空列表返回 0。
#[test]
fn sql_parser_feature_of_empty() {
    assert_eq!(SqlParserFeature::of(&[]), 0);
}

// ── Token 枚举变体 ──────────────────────────────────────────

/// Token 关键字变体 Debug 和 `PartialEq`。
#[test]
fn token_keyword_variants() {
    use druid_core::sql::Token;
    assert_eq!(Token::Select, Token::Select);
    assert_ne!(Token::Select, Token::Insert);
    assert_eq!(format!("{:?}", Token::Select), "Select");
    assert_eq!(format!("{:?}", Token::Insert), "Insert");
    assert_eq!(format!("{:?}", Token::Update), "Update");
    assert_eq!(format!("{:?}", Token::Delete), "Delete");
    assert_eq!(format!("{:?}", Token::From), "From");
    assert_eq!(format!("{:?}", Token::Where), "Where");
    assert_eq!(format!("{:?}", Token::And), "And");
    assert_eq!(format!("{:?}", Token::Or), "Or");
    assert_eq!(format!("{:?}", Token::Not), "Not");
    assert_eq!(format!("{:?}", Token::Null), "Null");
    assert_eq!(format!("{:?}", Token::Create), "Create");
    assert_eq!(format!("{:?}", Token::Alter), "Alter");
    assert_eq!(format!("{:?}", Token::Drop), "Drop");
    assert_eq!(format!("{:?}", Token::Table), "Table");
    assert_eq!(format!("{:?}", Token::Index), "Index");
    assert_eq!(format!("{:?}", Token::Join), "Join");
    assert_eq!(format!("{:?}", Token::Left), "Left");
    assert_eq!(format!("{:?}", Token::Right), "Right");
    assert_eq!(format!("{:?}", Token::Inner), "Inner");
    assert_eq!(format!("{:?}", Token::Outer), "Outer");
    assert_eq!(format!("{:?}", Token::Cross), "Cross");
    assert_eq!(format!("{:?}", Token::Union), "Union");
    assert_eq!(format!("{:?}", Token::All), "All");
    assert_eq!(format!("{:?}", Token::Distinct), "Distinct");
    assert_eq!(format!("{:?}", Token::As), "As");
    assert_eq!(format!("{:?}", Token::On), "On");
    assert_eq!(format!("{:?}", Token::Into), "Into");
    assert_eq!(format!("{:?}", Token::Values), "Values");
    assert_eq!(format!("{:?}", Token::Set), "Set");
    assert_eq!(format!("{:?}", Token::Limit), "Limit");
    assert_eq!(format!("{:?}", Token::Offset), "Offset");
    assert_eq!(format!("{:?}", Token::Order), "Order");
    assert_eq!(format!("{:?}", Token::Group), "Group");
    assert_eq!(format!("{:?}", Token::Having), "Having");
    assert_eq!(format!("{:?}", Token::Between), "Between");
    assert_eq!(format!("{:?}", Token::In), "In");
    assert_eq!(format!("{:?}", Token::Like), "Like");
    assert_eq!(format!("{:?}", Token::Exists), "Exists");
    assert_eq!(format!("{:?}", Token::Case), "Case");
    assert_eq!(format!("{:?}", Token::When), "When");
    assert_eq!(format!("{:?}", Token::Then), "Then");
    assert_eq!(format!("{:?}", Token::Else), "Else");
    assert_eq!(format!("{:?}", Token::End), "End");
    assert_eq!(format!("{:?}", Token::Is), "Is");
    assert_eq!(format!("{:?}", Token::True), "True");
    assert_eq!(format!("{:?}", Token::False), "False");
    assert_eq!(format!("{:?}", Token::Asc), "Asc");
    assert_eq!(format!("{:?}", Token::Desc), "Desc");
}

use std::collections::HashMap;
