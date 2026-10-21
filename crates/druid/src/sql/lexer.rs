//! Druid SQL 词法分析器。

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use num_bigint::BigInt;

use crate::core::RdbcString;

mod scan_a;
mod scan_b;

use super::{
    CharTypes, DbType, DialectFeature, DialectFeatureValue, Keywords, LayoutCharacters,
    LexerFeature, NotAllowCommentException, ParserException, SqlInsertNumber, SqlParserFeature,
    Token, DEFAULT_KEYWORDS, DM_KEYWORDS, SQLITE_KEYWORDS,
};

const FNV_BASIC: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 注释回调。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.Lexer.CommentHandler`。返回 `true`
/// 表示回调已接受该注释，但不会改变注释 Token 本身。
pub trait CommentHandler: Send + Sync {
    /// 处理刚扫描到的注释。
    ///
    /// `last_token` 对应 Java nullable `lastToken`，`comment` 保持原始 UTF-16。
    fn handle(&self, last_token: Option<Token>, comment: &RdbcString) -> bool;
}

/// Lexer 可回溯状态。
///
/// 对应 Java：`Lexer.SavePoint`。字段保存 Java `bp/sp/np/ch` 等状态，调用
/// [`Lexer::reset`] 后能从完全相同的 UTF-16 位置继续扫描。
#[derive(Debug, Clone)]
pub struct LexerSavePoint {
    pos: usize,
    start_pos: usize,
    buf_pos: usize,
    mark: usize,
    ch: u16,
    hash: i64,
    hash_l_case: i64,
    token: Option<Token>,
    string_val: Option<RdbcString>,
    line: usize,
}

/// Lexer 扫描错误。
///
/// Java 通过异常继承区分一般 parser 错误和禁止注释错误；Rust 用枚举保留这
/// 两个可观察分支。
#[derive(Debug)]
pub enum LexerError {
    /// 一般词法/解析错误。
    Parser(ParserException),
    /// 当前安全策略禁止 SQL 注释。
    NotAllowComment(NotAllowCommentException),
}

impl fmt::Display for LexerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parser(error) => error.fmt(formatter),
            Self::NotAllowComment(error) => error.fmt(formatter),
        }
    }
}

impl Error for LexerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parser(error) => Some(error),
            Self::NotAllowComment(error) => Some(error),
        }
    }
}

impl From<ParserException> for LexerError {
    fn from(value: ParserException) -> Self {
        Self::Parser(value)
    }
}

impl From<NotAllowCommentException> for LexerError {
    fn from(value: NotAllowCommentException) -> Self {
        Self::NotAllowComment(value)
    }
}

#[derive(Debug, Clone, Copy)]
enum KeywordSet {
    Default,
    SQLite,
    Dm,
}

/// Druid SQL UTF-16 词法分析器。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.Lexer`。本对象不以
/// `sqlparser-rs` tokenizer 替代 Druid Token：它直接在 Java String 等价的
/// UTF-16 code unit 上维护 `pos/mark/bufPos/ch/token/hash` 状态，并保留
/// SQLite、DM 关键字表、注释安全开关和 `SavePoint` 回溯语义。
pub struct Lexer {
    text: RdbcString,
    features: i32,
    pos: usize,
    mark: usize,
    number_scale: usize,
    number_exp: bool,
    ch: u16,
    buf_pos: usize,
    token: Option<Token>,
    keyword_set: KeywordSet,
    string_val: Option<RdbcString>,
    hash_l_case: i64,
    hash: i64,
    comment_count: usize,
    comments: Option<Vec<RdbcString>>,
    skip_comment: bool,
    save_point: Option<LexerSavePoint>,
    allow_comment: bool,
    var_index: i32,
    comment_handler: Option<Arc<dyn CommentHandler>>,
    end_of_comment: bool,
    keep_comments: bool,
    line: usize,
    lines: usize,
    db_type: Option<DbType>,
    optimized_for_parameterized: bool,
    keep_source_location: bool,
    start_pos: usize,
    pos_line: usize,
    pos_column: usize,
    dialect_feature: DialectFeature,
}

impl fmt::Debug for Lexer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Lexer")
            .field("utf16_length", &self.text.len())
            .field("pos", &self.pos)
            .field("mark", &self.mark)
            .field("buf_pos", &self.buf_pos)
            .field("token", &self.token)
            .field("db_type", &self.db_type)
            .field("line", &self.line)
            .finish_non_exhaustive()
    }
}

fn fnv_add(hash: i64, mut unit: u16, lower: bool) -> i64 {
    if lower && unit >= u16::from(b'A') && unit <= u16::from(b'Z') {
        unit += 32;
    }
    ((hash as u64) ^ u64::from(unit)).wrapping_mul(FNV_PRIME) as i64
}

fn fnv_units(units: &[u16], lower: bool) -> i64 {
    units
        .iter()
        .copied()
        .fold(FNV_BASIC as i64, |hash, unit| fnv_add(hash, unit, lower))
}
