//! Lexer 差分测试（C9 批次：sql/lexer.rs 698 行，0% → covered）。
//!
//! Java 基线：`com.alibaba.druid.sql.parser.Lexer`。

extern crate druid_core as druid;
use druid::core::RdbcString;
use druid::sql::{
    CommentHandler, DbType, Lexer, LexerError, LexerFeature, SqlInsertNumber, SqlParserFeature,
    Token,
};
use num_bigint::BigInt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── 构造函数 ───────────────────────────────────────────────────

/// `Lexer::new` 默认构造。
#[test]
fn lexer_new_default() {
    let lexer = Lexer::new("SELECT 1");
    assert!(lexer.token().is_none());
    assert_eq!(lexer.pos(), 0);
    assert_eq!(lexer.db_type(), None);
    assert!(lexer.string_val().is_none());
}

/// `Lexer::with_db_type` 指定方言。
#[test]
fn lexer_with_db_type() {
    let lexer = Lexer::with_db_type("SELECT 1", DbType::MySql);
    assert_eq!(lexer.db_type(), Some(DbType::MySql));
}

/// `Lexer::with_db_type` SQLite 使用 SQLite 关键字表。
#[test]
fn lexer_with_db_type_sqlite() {
    let lexer = Lexer::with_db_type("SELECT 1", DbType::SQLite);
    assert_eq!(lexer.db_type(), Some(DbType::SQLite));
    // SQLite 关键字表包含 SELECT。
    assert!(lexer.keywords().get_keyword("SELECT").is_some());
}

/// `Lexer::with_db_type` DM 使用 DM 关键字表。
#[test]
fn lexer_with_db_type_dm() {
    let lexer = Lexer::with_db_type("SELECT 1", DbType::Dm);
    assert_eq!(lexer.db_type(), Some(DbType::Dm));
}

/// `Lexer::from_rdbc_string` 从 UTF-16 构造。
#[test]
fn lexer_from_rdbc_string() {
    let rdbc = RdbcString::from_rust_str("SELECT 1");
    let lexer = Lexer::from_rdbc_string(rdbc, Some(DbType::PostgreSql), true);
    assert_eq!(lexer.db_type(), Some(DbType::PostgreSql));
    assert_eq!(lexer.source().to_rust_string().unwrap(), "SELECT 1");
}

// ── 基本 getter ────────────────────────────────────────────────

/// `source` 返回原始 SQL。
#[test]
fn lexer_source() {
    let lexer = Lexer::new("SELECT 1");
    assert_eq!(lexer.source().to_rust_string().unwrap(), "SELECT 1");
}

/// `char_at` 越界返回 EOI。
#[test]
fn lexer_char_at_out_of_bounds() {
    let lexer = Lexer::new("AB");
    assert_eq!(lexer.char_at(0), u16::from(b'A'));
    assert_eq!(lexer.char_at(1), u16::from(b'B'));
    // 越界返回 EOI (0x1A)。
    assert_eq!(lexer.char_at(100), u16::from(0x1Au8));
}

/// `keywords` 默认关键字表。
#[test]
fn lexer_keywords_default() {
    let lexer = Lexer::new("SELECT");
    let kws = lexer.keywords();
    assert!(kws.get_keyword("SELECT").is_some());
}

/// `Debug` 格式化不 panic。
#[test]
fn lexer_debug_format() {
    let lexer = Lexer::new("SELECT 1");
    let dbg = format!("{:?}", lexer);
    assert!(dbg.contains("Lexer"));
    assert!(dbg.contains("utf16_length"));
}

// ── next_token 基本扫描 ────────────────────────────────────────

/// 扫描 `SELECT` 关键字。
#[test]
fn lexer_scan_select_keyword() {
    let mut lexer = Lexer::new("SELECT");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Select);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        None
    );
}

/// 扫描标识符。
#[test]
fn lexer_scan_identifier() {
    let mut lexer = Lexer::new("my_table");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Identifier);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("my_table".to_owned())
    );
}

/// 扫描整数。
#[test]
fn lexer_scan_integer() {
    let mut lexer = Lexer::new("42");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralInt);
}

/// 扫描浮点数。
#[test]
fn lexer_scan_float() {
    let mut lexer = Lexer::new("3.14");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralFloat);
}

/// 扫描科学计数法。
#[test]
fn lexer_scan_scientific() {
    let mut lexer = Lexer::new("1e10");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralFloat);
}

/// 扫描科学计数法带符号。
#[test]
fn lexer_scan_scientific_with_sign() {
    let mut lexer = Lexer::new("1.5E-3");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralFloat);
}

/// 扫描十六进制。
#[test]
fn lexer_scan_hex() {
    let mut lexer = Lexer::new("0xFF");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralHex);
}

/// 扫描单引号字符串。
#[test]
fn lexer_scan_string_single_quote() {
    let mut lexer = Lexer::new("'hello'");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralChars);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("hello".to_owned())
    );
}

/// 扫描带转义的字符串。
#[test]
fn lexer_scan_string_with_escape() {
    let mut lexer = Lexer::new("'hello\\nworld'");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralChars);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("hello\nworld".to_owned())
    );
}

/// 扫描带转义制表符的字符串。
#[test]
fn lexer_scan_string_escape_tab() {
    let mut lexer = Lexer::new("'a\\tb'");
    lexer.next_token().unwrap();
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("a\tb".to_owned())
    );
}

/// 扫描带转义回车的字符串。
#[test]
fn lexer_scan_string_escape_cr() {
    let mut lexer = Lexer::new("'a\\rb'");
    lexer.next_token().unwrap();
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("a\rb".to_owned())
    );
}

/// 扫描带转义退格的字符串。
#[test]
fn lexer_scan_string_escape_backspace() {
    let mut lexer = Lexer::new("'a\\bb'");
    lexer.next_token().unwrap();
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("a\u{0008}b".to_owned())
    );
}

/// 扫描带转义 NUL 的字符串。
#[test]
fn lexer_scan_string_escape_nul() {
    let mut lexer = Lexer::new("'a\\0b'");
    lexer.next_token().unwrap();
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("a\u{0000}b".to_owned())
    );
}

/// 扫描带转义反斜杠的字符串。
#[test]
fn lexer_scan_string_escape_backslash() {
    let mut lexer = Lexer::new("'a\\\\b'");
    lexer.next_token().unwrap();
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("a\\b".to_owned())
    );
}

/// 扫描双引号标识符。
#[test]
fn lexer_scan_quoted_identifier_double_quote() {
    let mut lexer = Lexer::new("\"my column\"");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralAlias);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("my column".to_owned())
    );
}

/// 扫描反引号标识符。
#[test]
fn lexer_scan_quoted_identifier_backtick() {
    let mut lexer = Lexer::new("`my_column`");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Identifier);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("my_column".to_owned())
    );
}

/// 扫描 N 前缀字符串。
#[test]
fn lexer_scan_nchars_literal() {
    let mut lexer = Lexer::new("N'unicode'");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralNchars);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("unicode".to_owned())
    );
}

// ── 操作符扫描 ─────────────────────────────────────────────────

/// 扫描单字符操作符。
///
/// 注意：`@`/`#`/`$`/`:` 在 Druid Lexer 中优先走变量路径（`scan_variable`），
/// 不在此表中测试。
#[test]
fn lexer_scan_single_char_operators() {
    let cases = [
        ("=", Token::Eq),
        (">", Token::Gt),
        ("<", Token::Lt),
        ("+", Token::Plus),
        ("-", Token::Sub),
        ("*", Token::Star),
        ("/", Token::Slash),
        ("%", Token::Percent),
        ("&", Token::Amp),
        ("|", Token::Bar),
        ("^", Token::Caret),
        ("~", Token::Tilde),
        ("!", Token::Bang),
        ("?", Token::Ques),
        ("(", Token::Lparen),
        (")", Token::Rparen),
        ("{", Token::Lbrace),
        ("}", Token::Rbrace),
        ("[", Token::Lbracket),
        ("]", Token::Rbracket),
        (";", Token::Semi),
        (",", Token::Comma),
        (".", Token::Dot),
    ];
    for (text, expected) in cases {
        let mut lexer = Lexer::new(text);
        let token = lexer.next_token().unwrap();
        assert_eq!(token, expected, "operator {text:?}");
    }
}

/// 扫描双字符操作符。
#[test]
fn lexer_scan_two_char_operators() {
    let cases = [
        ("<>", Token::Ltgt),
        (">=", Token::Gteq),
        ("<=", Token::Lteq),
        ("!=", Token::Bangeq),
        ("||", Token::Barbar),
        ("&&", Token::Ampamp),
        ("::", Token::Coloncolon),
        (":=", Token::Coloneq),
        ("==", Token::Eqeq),
        ("=>", Token::Eqgt),
        ("->", Token::Subgt),
        ("<<", Token::Ltlt),
        (">>", Token::Gtgt),
        ("..", Token::Dotdot),
    ];
    for (text, expected) in cases {
        let mut lexer = Lexer::new(text);
        let token = lexer.next_token().unwrap();
        assert_eq!(token, expected, "operator {text:?}");
    }
}

/// 扫描全角括号和逗号。
#[test]
fn lexer_scan_fullwidth_symbols() {
    let mut lexer = Lexer::new("\u{FF08}\u{FF09}");
    assert_eq!(lexer.next_token().unwrap(), Token::Lparen);
    assert_eq!(lexer.next_token().unwrap(), Token::Rparen);
}

// ── 注释扫描 ───────────────────────────────────────────────────

/// 单行注释（--），skip_comment=false 时返回注释 token。
#[test]
fn lexer_scan_line_comment() {
    let mut lexer = Lexer::from_rdbc_string(
        RdbcString::from_rust_str("SELECT -- comment\n1"),
        None,
        false, // 不跳过注释
    );
    lexer.set_keep_comments(true);
    assert_eq!(lexer.next_token().unwrap(), Token::Select);
    assert_eq!(lexer.next_token().unwrap(), Token::LineComment);
    assert_eq!(lexer.comment_count(), 1);
    assert!(lexer.comments().is_some());
    assert!(lexer.is_end_of_comment());
}

/// 单行注释（//），skip_comment=false。
#[test]
fn lexer_scan_line_comment_slash_slash() {
    let mut lexer = Lexer::from_rdbc_string(RdbcString::from_rust_str("1 // comment"), None, false);
    lexer.set_keep_comments(true);
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
    assert_eq!(lexer.next_token().unwrap(), Token::LineComment);
    assert_eq!(lexer.comment_count(), 1);
}

/// 多行注释，skip_comment=false。
#[test]
fn lexer_scan_multi_line_comment() {
    let mut lexer =
        Lexer::from_rdbc_string(RdbcString::from_rust_str("/* block */ 1"), None, false);
    lexer.set_keep_comments(true);
    assert_eq!(lexer.next_token().unwrap(), Token::MultiLineComment);
    assert_eq!(lexer.comment_count(), 1);
    assert!(lexer.is_end_of_comment());
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
}

/// skip_comment=true 时注释被跳过。
#[test]
fn lexer_skip_comment() {
    let mut lexer = Lexer::from_rdbc_string(
        RdbcString::from_rust_str("-- comment\n1"),
        None,
        true, // skip_comment
    );
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
    assert_eq!(lexer.comment_count(), 1);
}

/// allow_comment=false 时注释抛出 NotAllowComment。
#[test]
fn lexer_not_allow_comment() {
    let mut lexer = Lexer::from_rdbc_string(
        RdbcString::from_rust_str("-- comment\n1"),
        None,
        false, // 不跳过注释，否则 allow_comment 检查不会触发
    );
    lexer.set_allow_comment(false);
    let result = lexer.next_token();
    assert!(result.is_err());
    match result.unwrap_err() {
        LexerError::NotAllowComment(_) => {}
        other => panic!("expected NotAllowComment, got {other:?}"),
    }
}

/// 嵌套多行注释，skip_comment=false。
#[test]
fn lexer_nested_multi_line_comment() {
    let mut lexer = Lexer::from_rdbc_string(
        RdbcString::from_rust_str("/* outer /* inner */ */ 1"),
        None,
        false,
    );
    assert_eq!(lexer.next_token().unwrap(), Token::MultiLineComment);
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
}

// ── 变量扫描 ───────────────────────────────────────────────────

/// 扫描 @ 变量。
#[test]
fn lexer_scan_at_variable() {
    let mut lexer = Lexer::new("@my_var");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("@my_var".to_owned())
    );
}

/// 扫描 @@ 变量（系统变量）。
#[test]
fn lexer_scan_double_at_variable() {
    let mut lexer = Lexer::new("@@global");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
}

/// 扫描 $ 变量。
#[test]
fn lexer_scan_dollar_variable() {
    let mut lexer = Lexer::new("$param");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
}

/// 扫描 $$ 变量。
#[test]
fn lexer_scan_double_dollar_variable() {
    let mut lexer = Lexer::new("$$func");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
}

/// 扫描 # 变量。
#[test]
fn lexer_scan_hash_variable() {
    let mut lexer = Lexer::new("#col");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
}

/// 扫描 : 变量（非 :: 非 :=）。
#[test]
fn lexer_scan_colon_variable() {
    let mut lexer = Lexer::new(":param");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
}

/// 扫描 ${} 变量。
#[test]
fn lexer_scan_braced_variable() {
    let mut lexer = Lexer::new("${env_var}");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Variant);
    assert_eq!(
        lexer.string_val().map(|s| s.to_rust_string().unwrap()),
        Some("${env_var}".to_owned())
    );
}

// ── next_var_index ─────────────────────────────────────────────

/// `next_var_index` 递增。
#[test]
fn lexer_next_var_index() {
    let mut lexer = Lexer::new("SELECT 1");
    assert_eq!(lexer.next_var_index(), 0);
    assert_eq!(lexer.next_var_index(), 1);
    assert_eq!(lexer.next_var_index(), 2);
}

// ── next_if ────────────────────────────────────────────────────

/// `next_if` 匹配时前进。
#[test]
fn lexer_next_if_match() {
    let mut lexer = Lexer::new("SELECT 1");
    assert_eq!(lexer.next_token().unwrap(), Token::Select);
    assert!(lexer.next_if(Token::Select).unwrap());
    assert_eq!(lexer.token(), Some(Token::LiteralInt));
}

/// `next_if` 不匹配时不动。
#[test]
fn lexer_next_if_no_match() {
    let mut lexer = Lexer::new("SELECT 1");
    assert_eq!(lexer.next_token().unwrap(), Token::Select);
    assert!(!lexer.next_if(Token::From).unwrap());
    assert_eq!(lexer.token(), Some(Token::Select));
}

// ── skip_to_eof ────────────────────────────────────────────────

/// `skip_to_eof` 跳到末尾。
#[test]
fn lexer_skip_to_eof() {
    let mut lexer = Lexer::new("SELECT * FROM t");
    lexer.next_token().unwrap();
    lexer.skip_to_eof();
    assert_eq!(lexer.token(), Some(Token::Eof));
}

// ── save point（mark / reset）──────────────────────────────────

/// `mark_out` + `reset` 回溯。
#[test]
fn lexer_save_point_roundtrip() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.next_token().unwrap();
    let save = lexer.mark_out();
    assert_eq!(lexer.token(), Some(Token::Select));

    lexer.next_token().unwrap();
    assert_eq!(lexer.token(), Some(Token::LiteralInt));

    lexer.reset(&save);
    assert_eq!(lexer.token(), Some(Token::Select));
}

/// `mark` + `reset_saved` 回溯。
#[test]
fn lexer_mark_and_reset_saved() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.next_token().unwrap();
    let _ = lexer.mark();
    lexer.next_token().unwrap();
    assert_eq!(lexer.token(), Some(Token::LiteralInt));

    lexer.reset_saved();
    assert_eq!(lexer.token(), Some(Token::Select));
}

/// `reset_pos` 直接跳转。
#[test]
fn lexer_reset_pos() {
    let mut lexer = Lexer::new("AB");
    lexer.reset_pos(1);
    assert_eq!(lexer.char_at(lexer.pos()), u16::from(b'B'));
}

// ── set_token ──────────────────────────────────────────────────

/// `set_token` 强制设置。
#[test]
fn lexer_set_token() {
    let mut lexer = Lexer::new("SELECT");
    lexer.next_token().unwrap();
    lexer.set_token(Token::Eof);
    assert_eq!(lexer.token(), Some(Token::Eof));
}

// ── token_start / token_len ────────────────────────────────────

/// `token_start` 和 `token_len`。
#[test]
fn lexer_token_position() {
    let mut lexer = Lexer::new("  SELECT");
    lexer.next_token().unwrap();
    assert_eq!(lexer.token_start(), 2);
    assert_eq!(lexer.token_len(), 6);
}

// ── hash ───────────────────────────────────────────────────────

/// `hash` 和 `hash_l_case` 对标识符非零。
#[test]
fn lexer_hash_nonzero() {
    let mut lexer = Lexer::new("myIdent");
    lexer.next_token().unwrap();
    assert_ne!(lexer.hash(), 0);
    assert_ne!(lexer.hash_l_case(), 0);
}

// ── integer_value ──────────────────────────────────────────────

/// 整数转换为 Integer。
#[test]
fn lexer_integer_value_int() {
    let mut lexer = Lexer::new("42");
    lexer.next_token().unwrap();
    match lexer.integer_value().unwrap() {
        SqlInsertNumber::Integer(v) => assert_eq!(v, 42),
        other => panic!("expected Integer, got {other:?}"),
    }
}

/// 大整数转换为 Long。
#[test]
fn lexer_integer_value_long() {
    let mut lexer = Lexer::new("9999999999");
    lexer.next_token().unwrap();
    match lexer.integer_value().unwrap() {
        SqlInsertNumber::Long(v) => assert_eq!(v, 9_999_999_999),
        other => panic!("expected Long, got {other:?}"),
    }
}

/// 十六进制整数（原始文本含 0x 前缀，parse::<i32> 失败后回退 BigInteger）。
#[test]
fn lexer_integer_value_hex() {
    let mut lexer = Lexer::new("0xFF");
    lexer.next_token().unwrap();
    match lexer.integer_value().unwrap() {
        SqlInsertNumber::BigInteger(v) => assert_eq!(v, BigInt::from(255)),
        other => panic!("expected BigInteger, got {other:?}"),
    }
}

// ── config / is_enabled ────────────────────────────────────────

/// `config` 设置 feature。
#[test]
fn lexer_config_feature() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.config(SqlParserFeature::KeepComments, true);
    assert!(lexer.is_enabled(SqlParserFeature::KeepComments));
    lexer.config(SqlParserFeature::KeepComments, false);
    assert!(!lexer.is_enabled(SqlParserFeature::KeepComments));
}

/// `config` OptimizedForParameterized 设置内部标志。
#[test]
fn lexer_config_optimized_for_parameterized() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.config(SqlParserFeature::OptimizedForParameterized, true);
    assert!(lexer.is_enabled(SqlParserFeature::OptimizedForParameterized));
}

/// `config` KeepSourceLocation。
#[test]
fn lexer_config_keep_source_location() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.config(SqlParserFeature::KeepSourceLocation, true);
    assert!(lexer.is_enabled(SqlParserFeature::KeepSourceLocation));
}

/// `config` SkipComments。
#[test]
fn lexer_config_skip_comments() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.config(SqlParserFeature::SkipComments, true);
    assert!(lexer.is_enabled(SqlParserFeature::SkipComments));
}

// ── dialect feature ────────────────────────────────────────────

/// `config_lexer_feature` + `dialect_feature_enabled`。
#[test]
fn lexer_dialect_feature() {
    let mut lexer = Lexer::new("SELECT 1");
    // ScanNumberPrefixB 默认启用（Java 默认配置）。
    assert!(lexer.dialect_feature_enabled(LexerFeature::ScanNumberPrefixB));
    lexer.config_lexer_feature(LexerFeature::ScanNumberPrefixB, false);
    assert!(!lexer.dialect_feature_enabled(LexerFeature::ScanNumberPrefixB));
}

// ── comment handler ────────────────────────────────────────────

/// `set_comment_handler` 回调被调用。
#[test]
fn lexer_comment_handler() {
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&count);
    struct CountHandler(Arc<AtomicUsize>);
    impl CommentHandler for CountHandler {
        fn handle(&self, _last_token: Option<Token>, _comment: &RdbcString) -> bool {
            self.0.fetch_add(1, Ordering::Relaxed);
            true
        }
    }
    let mut lexer = Lexer::from_rdbc_string(
        RdbcString::from_rust_str("/* c1 */ /* c2 */ 1"),
        None,
        false, // don't skip
    );
    lexer.set_comment_handler(Some(Arc::new(CountHandler(count_clone))));
    lexer.next_token().unwrap(); // first comment
    lexer.next_token().unwrap(); // second comment
    assert_eq!(count.load(Ordering::Relaxed), 2);
}

// ── info ───────────────────────────────────────────────────────

/// `info` 返回诊断字符串。
#[test]
fn lexer_info() {
    let mut lexer = Lexer::new("SELECT 1");
    lexer.next_token().unwrap();
    let info = lexer.info();
    assert!(info.contains("pos"));
    assert!(info.contains("line"));
    assert!(info.contains("token"));
}

// ── EOF ────────────────────────────────────────────────────────

/// 空输入直接 EOF。
#[test]
fn lexer_empty_input_eof() {
    let mut lexer = Lexer::new("");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Eof);
}

/// 空白输入直接 EOF。
#[test]
fn lexer_whitespace_only_eof() {
    let mut lexer = Lexer::new("   \t\n  ");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Eof);
}

// ── 行号追踪 ───────────────────────────────────────────────────

/// 换行递增行号。
#[test]
fn lexer_line_tracking() {
    let mut lexer = Lexer::new("SELECT\n1");
    lexer.next_token().unwrap(); // SELECT
    lexer.next_token().unwrap(); // 1
                                 // info 应该反映在第二行。
    let info = lexer.info();
    assert!(info.contains("line 2"), "info: {info}");
}

// ── 全角逗号 ───────────────────────────────────────────────────

/// 全角逗号扫描为 Comma。
#[test]
fn lexer_fullwidth_comma() {
    let mut lexer = Lexer::new("\u{FF0C}");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Comma);
}

// ── NCHAR 前缀（小写 n）───────────────────────────────────────

/// 小写 n 前缀字符串。
#[test]
fn lexer_nchars_lowercase() {
    let mut lexer = Lexer::new("n'test'");
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::LiteralNchars);
}

// ── 字符串内换行 ───────────────────────────────────────────────

/// 字符串内换行递增行号。
#[test]
fn lexer_string_with_newline() {
    let mut lexer = Lexer::new("'line1\nline2'");
    lexer.next_token().unwrap();
    let info = lexer.info();
    assert!(info.contains("line 2"), "info: {info}");
}

// ── 多 token 扫描序列 ─────────────────────────────────────────

/// `SELECT * FROM t WHERE id = 1` 完整序列。
#[test]
fn lexer_full_select_sequence() {
    let mut lexer = Lexer::new("SELECT * FROM t WHERE id = 1");
    assert_eq!(lexer.next_token().unwrap(), Token::Select);
    assert_eq!(lexer.next_token().unwrap(), Token::Star);
    assert_eq!(lexer.next_token().unwrap(), Token::From);
    assert_eq!(lexer.next_token().unwrap(), Token::Identifier);
    assert_eq!(lexer.next_token().unwrap(), Token::Where);
    assert_eq!(lexer.next_token().unwrap(), Token::Identifier);
    assert_eq!(lexer.next_token().unwrap(), Token::Eq);
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
    assert_eq!(lexer.next_token().unwrap(), Token::Eof);
}

/// em-dash 分隔符被跳过。
#[test]
fn lexer_em_dash_separator() {
    let mut lexer = Lexer::new("\u{2014}\u{2014}\nSELECT 1");
    assert_eq!(lexer.next_token().unwrap(), Token::Select);
}

// ── 负数 ───────────────────────────────────────────────────────

/// 负数在逗号后扫描为 LiteralInt。
#[test]
fn lexer_negative_after_comma() {
    let mut lexer = Lexer::new("(-1)");
    assert_eq!(lexer.next_token().unwrap(), Token::Lparen);
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
}

// ── 二进制位字面量 ─────────────────────────────────────────────

/// 0b 前缀（需启用 ScanNumberPrefixB）。
#[test]
fn lexer_bits_literal() {
    let mut lexer = Lexer::new("0b1010");
    lexer.config_lexer_feature(LexerFeature::ScanNumberPrefixB, true);
    let token = lexer.next_token().unwrap();
    assert_eq!(token, Token::Bits);
}

// ── 多行注释未闭合 ─────────────────────────────────────────────

/// 未闭合多行注释返回错误。
#[test]
fn lexer_unterminated_comment() {
    let mut lexer = Lexer::new("/* unterminated");
    let result = lexer.next_token();
    assert!(result.is_err());
}

// ── 引号标识符未闭合 ───────────────────────────────────────────

/// 未闭合双引号标识符返回错误。
#[test]
fn lexer_unterminated_quoted_identifier() {
    let mut lexer = Lexer::new("\"unterminated");
    let result = lexer.next_token();
    assert!(result.is_err());
}

// ── 字符串未闭合 ───────────────────────────────────────────────

/// 未闭合单引号字符串返回错误。
#[test]
fn lexer_unterminated_string() {
    let mut lexer = Lexer::new("'unterminated");
    let result = lexer.next_token();
    assert!(result.is_err());
}

// ── 非法字符 ───────────────────────────────────────────────────

/// 非法字符（不在空白/标识符/操作符表中的字符）返回 LexerError::Parser。
///
/// `\u{00A1}`（¡）在 Latin-1 Supplement 区间 0xA1-0xBF，不是字母（0xC0+）、
/// 不是空白（≤32 或 0x7F-0xA0）、不是操作符、不是标识符首字符。
#[test]
fn lexer_illegal_char() {
    let mut lexer = Lexer::new("\u{00A1}");
    let result = lexer.next_token();
    assert!(result.is_err());
    match result.unwrap_err() {
        LexerError::Parser(_) => {}
        other => panic!("expected Parser error, got {other:?}"),
    }
}

// ── EOF 后重复调用 ─────────────────────────────────────────────

/// EOF 后再调用仍返回 EOF。
#[test]
fn lexer_repeated_eof() {
    let mut lexer = Lexer::new("1");
    assert_eq!(lexer.next_token().unwrap(), Token::LiteralInt);
    assert_eq!(lexer.next_token().unwrap(), Token::Eof);
    assert_eq!(lexer.next_token().unwrap(), Token::Eof);
}
