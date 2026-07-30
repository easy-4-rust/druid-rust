//! Druid SQL 词法分析器。

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use num_bigint::BigInt;

use crate::core::JavaString;

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
    fn handle(&self, last_token: Option<Token>, comment: &JavaString) -> bool;
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
    string_val: Option<JavaString>,
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
/// SQLite、DM 关键字表、注释安全开关和 SavePoint 回溯语义。
pub struct Lexer {
    text: JavaString,
    features: i32,
    pos: usize,
    mark: usize,
    number_scale: usize,
    number_exp: bool,
    ch: u16,
    buf_pos: usize,
    token: Option<Token>,
    keyword_set: KeywordSet,
    string_val: Option<JavaString>,
    hash_l_case: i64,
    hash: i64,
    comment_count: usize,
    comments: Option<Vec<JavaString>>,
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

impl Lexer {
    /// 从 Rust UTF-8 SQL 创建默认 Lexer。
    #[must_use]
    pub fn new(input: impl AsRef<str>) -> Self {
        Self::from_java_string(JavaString::from_rust_str(input.as_ref()), None, true)
    }

    /// 从 SQL 与数据库类型创建 Lexer。
    #[must_use]
    pub fn with_db_type(input: impl AsRef<str>, db_type: DbType) -> Self {
        Self::from_java_string(
            JavaString::from_rust_str(input.as_ref()),
            Some(db_type),
            true,
        )
    }

    /// 从无损 Java UTF-16 SQL 创建 Lexer。
    #[must_use]
    pub fn from_java_string(text: JavaString, db_type: Option<DbType>, skip_comment: bool) -> Self {
        let keyword_set = match db_type {
            Some(DbType::SQLite) => KeywordSet::SQLite,
            Some(DbType::Dm) => KeywordSet::Dm,
            _ => KeywordSet::Default,
        };
        let mut lexer = Self {
            text,
            features: 0,
            pos: 0,
            mark: 0,
            number_scale: 0,
            number_exp: false,
            ch: u16::from(LayoutCharacters::EOI),
            buf_pos: 0,
            token: None,
            keyword_set,
            string_val: None,
            hash_l_case: 0,
            hash: 0,
            comment_count: 0,
            comments: None,
            skip_comment,
            save_point: None,
            allow_comment: true,
            var_index: -1,
            comment_handler: None,
            end_of_comment: false,
            keep_comments: false,
            line: 0,
            lines: 0,
            db_type,
            optimized_for_parameterized: false,
            keep_source_location: false,
            start_pos: 0,
            pos_line: 0,
            pos_column: 0,
            dialect_feature: DialectFeature::new(),
        };
        lexer.ch = lexer.char_at(0);
        while lexer.ch == 0x200B || lexer.ch == u16::from(b'\n') {
            if lexer.ch == u16::from(b'\n') {
                lexer.line += 1;
            }
            lexer.pos += 1;
            lexer.ch = lexer.char_at(lexer.pos);
        }
        lexer
    }

    /// 返回原始 Java String。
    #[must_use]
    pub const fn source(&self) -> &JavaString {
        &self.text
    }

    /// 返回指定 UTF-16 下标的 code unit；越界返回 Java EOI。
    #[must_use]
    pub fn char_at(&self, index: usize) -> u16 {
        self.text
            .as_utf16()
            .get(index)
            .copied()
            .unwrap_or(u16::from(LayoutCharacters::EOI))
    }

    /// 返回当前 Token；首次扫描前为 `None`，对应 Java null。
    #[must_use]
    pub const fn token(&self) -> Option<Token> {
        self.token
    }

    /// 强制设置当前 Token。
    pub fn set_token(&mut self, token: Token) {
        self.token = Some(token);
    }

    /// 返回当前 Token 对应的 UTF-16 值。
    #[must_use]
    pub const fn string_val(&self) -> Option<&JavaString> {
        self.string_val.as_ref()
    }

    /// 返回当前扫描位置。
    #[must_use]
    pub const fn pos(&self) -> usize {
        self.pos
    }

    /// 返回当前 Token 起点。
    #[must_use]
    pub const fn token_start(&self) -> usize {
        self.mark
    }

    /// 返回当前 Token UTF-16 长度。
    #[must_use]
    pub const fn token_len(&self) -> usize {
        self.buf_pos
    }

    /// 返回当前 Token 的大小写敏感 FNV-1a hash。
    #[must_use]
    pub const fn hash(&self) -> i64 {
        self.hash
    }

    /// 返回当前 Token 的 ASCII-lower FNV-1a hash。
    #[must_use]
    pub const fn hash_l_case(&self) -> i64 {
        self.hash_l_case
    }

    /// 返回数据库类型。
    #[must_use]
    pub const fn db_type(&self) -> Option<DbType> {
        self.db_type
    }

    /// 返回当前关键字集合。
    #[must_use]
    pub fn keywords(&self) -> &'static Keywords {
        match self.keyword_set {
            KeywordSet::Default => &DEFAULT_KEYWORDS,
            KeywordSet::SQLite => &SQLITE_KEYWORDS,
            KeywordSet::Dm => &DM_KEYWORDS,
        }
    }

    /// 设置注释回调。
    pub fn set_comment_handler(&mut self, handler: Option<Arc<dyn CommentHandler>>) {
        self.comment_handler = handler;
    }

    /// 设置是否允许注释。
    pub fn set_allow_comment(&mut self, allow_comment: bool) {
        self.allow_comment = allow_comment;
    }

    /// 返回是否允许注释。
    #[must_use]
    pub const fn is_allow_comment(&self) -> bool {
        self.allow_comment
    }

    /// 设置是否保留注释。
    pub fn set_keep_comments(&mut self, keep_comments: bool) {
        self.keep_comments = keep_comments;
    }

    /// 返回是否保留注释。
    #[must_use]
    pub const fn is_keep_comments(&self) -> bool {
        self.keep_comments
    }

    /// 返回累计扫描的注释数量。
    #[must_use]
    pub const fn comment_count(&self) -> usize {
        self.comment_count
    }

    /// 返回已保留的注释。
    #[must_use]
    pub fn comments(&self) -> Option<&[JavaString]> {
        self.comments.as_deref()
    }

    /// 返回是否到达注释结尾。
    #[must_use]
    pub const fn is_end_of_comment(&self) -> bool {
        self.end_of_comment
    }

    /// 递增并返回变量序号。
    pub fn next_var_index(&mut self) -> i32 {
        self.var_index += 1;
        self.var_index
    }

    /// 配置 Java SQLParserFeature mask 及关联快捷字段。
    pub fn config(&mut self, feature: SqlParserFeature, state: bool) {
        self.features = SqlParserFeature::config(self.features, feature, state);
        match feature {
            SqlParserFeature::OptimizedForParameterized => {
                self.optimized_for_parameterized = state;
            }
            SqlParserFeature::KeepComments => self.keep_comments = state,
            SqlParserFeature::KeepSourceLocation => self.keep_source_location = state,
            SqlParserFeature::SkipComments => self.skip_comment = state,
            _ => {}
        }
    }

    /// 判断 SQLParserFeature 是否启用。
    #[must_use]
    pub const fn is_enabled(&self, feature: SqlParserFeature) -> bool {
        SqlParserFeature::is_enabled(self.features, feature)
    }

    /// 打开或关闭 Lexer 方言特性。
    pub fn config_lexer_feature(&mut self, feature: LexerFeature, state: bool) {
        self.dialect_feature
            .config_feature(DialectFeatureValue::Lexer(feature), state);
    }

    /// 判断 Lexer 方言特性是否启用。
    #[must_use]
    pub const fn dialect_feature_enabled(&self, feature: LexerFeature) -> bool {
        self.dialect_feature
            .is_enabled(DialectFeatureValue::Lexer(feature))
    }

    /// 导出当前完整回溯点。
    #[must_use]
    pub fn mark_out(&self) -> LexerSavePoint {
        LexerSavePoint {
            pos: self.pos,
            start_pos: self.start_pos,
            buf_pos: self.buf_pos,
            mark: self.mark,
            ch: self.ch,
            hash: self.hash,
            hash_l_case: self.hash_l_case,
            token: self.token,
            string_val: self.string_val.clone(),
            line: self.line,
        }
    }

    /// 保存并返回兼容 Java deprecated `mark()` 的内部回溯点。
    pub fn mark(&mut self) -> LexerSavePoint {
        let save_point = self.mark_out();
        self.save_point = Some(save_point.clone());
        save_point
    }

    /// 恢复指定回溯点的全部扫描状态。
    pub fn reset(&mut self, save_point: &LexerSavePoint) {
        self.pos = save_point.pos;
        self.start_pos = save_point.start_pos;
        self.buf_pos = save_point.buf_pos;
        self.mark = save_point.mark;
        self.ch = save_point.ch;
        self.hash = save_point.hash;
        self.hash_l_case = save_point.hash_l_case;
        self.token = save_point.token;
        self.string_val = save_point.string_val.clone();
        self.line = save_point.line;
    }

    /// 恢复内部 `mark()` 保存的回溯点。
    pub fn reset_saved(&mut self) {
        if let Some(save_point) = self.save_point.clone() {
            self.reset(&save_point);
        }
    }

    /// 直接跳转到指定 UTF-16 下标。
    pub fn reset_pos(&mut self, pos: usize) {
        self.pos = pos;
        self.ch = self.char_at(pos);
    }

    /// 如果当前 Token 匹配则扫描下一 Token。
    pub fn next_if(&mut self, token: Token) -> Result<bool, LexerError> {
        if self.token == Some(token) {
            self.next_token()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 直接移动到输入末尾。
    pub fn skip_to_eof(&mut self) {
        self.pos = self.text.len();
        self.ch = u16::from(LayoutCharacters::EOI);
        self.token = Some(Token::Eof);
    }

    /// 返回当前位置的人类可读诊断信息。
    #[must_use]
    pub fn info(&mut self) -> String {
        self.compute_row_and_column();
        let token = self
            .token
            .map_or("null", |value| value.name().unwrap_or(value.java_name()));
        format!(
            "pos {}, line {}, column {}, token {}",
            self.pos, self.pos_line, self.pos_column, token
        )
    }

    /// 扫描下一个 Druid Token。
    ///
    /// 对应 Java：`Lexer#nextToken()` 的通用路径：空白、标识符/关键字、字符串、
    /// 数字、变量、注释、全套公共操作符及全角括号/逗号均在同一状态机中处理。
    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        self.start_pos = self.pos;
        self.buf_pos = 0;
        self.string_val = None;
        if self
            .comments
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        {
            self.comments = None;
        }
        self.lines = 0;
        let start_line = self.line;

        loop {
            while CharTypes::is_whitespace(self.ch) {
                if self.ch == u16::from(b'\n') {
                    self.line += 1;
                    self.lines = self.line - start_line;
                }
                self.scan_char();
                self.start_pos = self.pos;
            }

            // Java String 可以真实包含 EOI code unit；只有越界时才是 EOF。
            if self.ch == u16::from(LayoutCharacters::EOI) && self.pos < self.text.len() {
                self.scan_char();
                continue;
            }
            if self.is_eof() {
                return Ok(self.finish_simple(Token::Eof, self.pos, 0));
            }

            if self.ch == u16::from(b'N') || self.ch == u16::from(b'n') {
                if self.char_at(self.pos + 1) == u16::from(b'\'') {
                    self.pos += 1;
                    self.ch = u16::from(b'\'');
                    self.scan_string(Token::LiteralNchars)?;
                    return Ok(Token::LiteralNchars);
                }
            }

            // Java CharTypes 会把全角左括号判为 identifier 首字符，Lexer 在进入
            // scanIdentifier 前显式纠正为括号；该顺序是可观察语义。
            if self.ch == 0xFF08 || self.ch == 0xFF09 {
                let token = if self.ch == 0xFF08 {
                    Token::Lparen
                } else {
                    Token::Rparen
                };
                let start = self.pos;
                self.scan_char();
                return Ok(self.finish_simple(token, start, 1));
            }

            // Druid 接受仅由两个 em dash 加换行组成的历史分隔符并直接跳过。
            if self.ch == 0x2014
                && self.char_at(self.pos + 1) == 0x2014
                && self.char_at(self.pos + 2) == u16::from(b'\n')
            {
                self.pos += 3;
                self.ch = self.char_at(self.pos);
                self.line += 1;
                continue;
            }

            if self.ch == u16::from(b'\'') {
                self.scan_string(Token::LiteralChars)?;
                return Ok(Token::LiteralChars);
            }
            if self.ch == u16::from(b'"') {
                self.scan_quoted_identifier(u16::from(b'"'), Token::LiteralAlias)?;
                return Ok(Token::LiteralAlias);
            }
            if self.ch == u16::from(b'`') {
                self.scan_quoted_identifier(u16::from(b'`'), Token::Identifier)?;
                return Ok(Token::Identifier);
            }

            if CharTypes::is_digit(self.ch)
                || (self.ch == u16::from(b'.') && CharTypes::is_digit(self.char_at(self.pos + 1)))
                || (self.ch == u16::from(b'-')
                    && CharTypes::is_digit(self.char_at(self.pos + 1))
                    && matches!(
                        self.token,
                        None | Some(Token::Comma | Token::Lparen | Token::With | Token::By)
                    ))
            {
                let token = self.scan_number();
                return Ok(token);
            }

            if (self.ch == u16::from(b'-') && self.char_at(self.pos + 1) == u16::from(b'-'))
                || (self.ch == u16::from(b'/')
                    && matches!(
                        self.char_at(self.pos + 1),
                        value if value == u16::from(b'/') || value == u16::from(b'*')
                    ))
            {
                let token = self.scan_comment()?;
                if self.skip_comment {
                    continue;
                }
                return Ok(token);
            }

            if self.ch == u16::from(b'$')
                || self.ch == u16::from(b'@')
                || self.ch == u16::from(b'#')
                || (self.ch == u16::from(b':')
                    && !matches!(
                        self.char_at(self.pos + 1),
                        value if value == u16::from(b':') || value == u16::from(b'=')
                    ))
            {
                self.scan_variable();
                return Ok(Token::Variant);
            }

            if CharTypes::is_first_identifier_char(self.ch) {
                self.scan_identifier();
                return Ok(self.token.unwrap_or(Token::Identifier));
            }

            if let Some(token) = self.scan_operator() {
                return Ok(token);
            }

            let illegal = self.ch;
            self.scan_char();
            self.token = Some(Token::Error);
            return Err(ParserException::with_position(
                format!("illegal.char, {}", illegal),
                self.line as i32,
                self.pos_column as i32,
            )
            .into());
        }
    }

    /// 将当前整数字面量转换为 Java Integer/Long/BigInteger 等价值。
    pub fn integer_value(&self) -> Result<SqlInsertNumber, ParserException> {
        let value = self.token_slice();
        let rust_value = String::from_utf16(value).map_err(|error| {
            ParserException::with_message(format!("invalid integer UTF-16: {error}"))
        })?;
        let radix = if self.token == Some(Token::LiteralHex) {
            16
        } else {
            10
        };
        let digits = rust_value
            .strip_prefix("0x")
            .or_else(|| rust_value.strip_prefix("0X"))
            .unwrap_or(&rust_value);
        let parsed = BigInt::parse_bytes(digits.as_bytes(), radix).ok_or_else(|| {
            ParserException::with_message(format!("illegal integer {rust_value}"))
        })?;
        if let Ok(value) = rust_value.parse::<i32>() {
            Ok(SqlInsertNumber::Integer(value))
        } else if let Ok(value) = rust_value.parse::<i64>() {
            Ok(SqlInsertNumber::Long(value))
        } else {
            Ok(SqlInsertNumber::BigInteger(parsed))
        }
    }

    fn scan_identifier(&mut self) {
        self.mark = self.pos;
        self.hash = FNV_BASIC as i64;
        self.hash_l_case = FNV_BASIC as i64;
        while CharTypes::is_identifier_char(self.ch) {
            self.hash = fnv_add(self.hash, self.ch, false);
            self.hash_l_case = fnv_add(self.hash_l_case, self.ch, true);
            self.scan_char();
        }
        self.buf_pos = self.pos - self.mark;
        let value = self.slice(self.mark, self.buf_pos);
        self.token = self
            .keywords()
            .get_keyword_java_string(&value)
            .or(Some(Token::Identifier));
        if self.token == Some(Token::Identifier) {
            self.string_val = Some(value);
        }
    }

    fn scan_quoted_identifier(&mut self, quote: u16, token: Token) -> Result<(), ParserException> {
        self.mark = self.pos;
        self.scan_char();
        let value_start = self.pos;
        let mut value = Vec::new();
        loop {
            if self.is_eof() {
                return Err(ParserException::with_position(
                    "illegal identifier",
                    self.line as i32,
                    self.pos_column as i32,
                ));
            }
            if self.ch == quote {
                if self.char_at(self.pos + 1) == quote {
                    value.push(quote);
                    self.scan_char();
                    self.scan_char();
                    continue;
                }
                self.scan_char();
                break;
            }
            value.push(self.ch);
            self.scan_char();
        }
        self.buf_pos = self.pos - self.mark;
        self.hash = fnv_units(&value, false);
        self.hash_l_case = fnv_units(&value, true);
        self.string_val = Some(if value.is_empty() && self.buf_pos > 2 {
            self.slice(value_start, self.buf_pos - 2)
        } else {
            JavaString::from_utf16(value)
        });
        self.token = Some(token);
        Ok(())
    }

    fn scan_string(&mut self, token: Token) -> Result<(), ParserException> {
        self.mark = self.pos;
        self.scan_char();
        let mut value = Vec::new();
        loop {
            if self.is_eof() {
                return Err(ParserException::with_position(
                    "unclosed.str.lit",
                    self.line as i32,
                    self.pos_column as i32,
                ));
            }
            if self.ch == u16::from(b'\'') {
                if self.char_at(self.pos + 1) == u16::from(b'\'') {
                    value.push(u16::from(b'\''));
                    self.scan_char();
                    self.scan_char();
                    continue;
                }
                self.scan_char();
                break;
            }
            if self.ch == u16::from(b'\\') {
                let escaped = self.char_at(self.pos + 1);
                if escaped != u16::from(LayoutCharacters::EOI) {
                    self.scan_char();
                    value.push(match self.ch {
                        value if value == u16::from(b'n') => u16::from(b'\n'),
                        value if value == u16::from(b'r') => u16::from(b'\r'),
                        value if value == u16::from(b't') => u16::from(b'\t'),
                        value if value == u16::from(b'b') => 8,
                        value if value == u16::from(b'0') => 0,
                        value => value,
                    });
                    self.scan_char();
                    continue;
                }
            }
            if self.ch == u16::from(b'\n') {
                self.line += 1;
            }
            value.push(self.ch);
            self.scan_char();
        }
        self.buf_pos = self.pos - self.mark;
        self.string_val = Some(JavaString::from_utf16(value));
        self.token = Some(token);
        Ok(())
    }

    fn scan_number(&mut self) -> Token {
        self.mark = self.pos;
        self.number_scale = 0;
        self.number_exp = false;
        if self.ch == u16::from(b'-') {
            self.scan_char();
        }
        if self.ch == u16::from(b'0')
            && self.char_at(self.pos + 1) == u16::from(b'b')
            && self.dialect_feature_enabled(LexerFeature::ScanNumberPrefixB)
        {
            self.scan_char();
            self.scan_char();
            let bits_start = self.pos;
            while self.ch == u16::from(b'0') || self.ch == u16::from(b'1') {
                self.scan_char();
            }
            if !CharTypes::is_digit(self.ch) {
                self.buf_pos = self.pos - self.mark;
                self.string_val = Some(self.slice(bits_start, self.pos - bits_start));
                self.token = Some(Token::Bits);
                return Token::Bits;
            }
        }
        if self.ch == u16::from(b'0')
            && matches!(
                self.char_at(self.pos + 1),
                value if value == u16::from(b'x') || value == u16::from(b'X')
            )
        {
            self.scan_char();
            self.scan_char();
            while CharTypes::is_hex(self.ch) {
                self.scan_char();
            }
            self.buf_pos = self.pos - self.mark;
            self.token = Some(Token::LiteralHex);
            return Token::LiteralHex;
        }
        while CharTypes::is_digit(self.ch) {
            self.scan_char();
        }
        if self.ch == u16::from(b'.') && self.char_at(self.pos + 1) != u16::from(b'.') {
            self.number_exp = true;
            self.scan_char();
            while CharTypes::is_digit(self.ch) {
                self.number_scale += 1;
                self.scan_char();
            }
        }
        if matches!(self.ch, value if value == u16::from(b'e') || value == u16::from(b'E')) {
            let first = self.char_at(self.pos + 1);
            let second = self.char_at(self.pos + 2);
            if CharTypes::is_digit(first)
                || ((first == u16::from(b'+') || first == u16::from(b'-'))
                    && CharTypes::is_digit(second))
            {
                self.number_exp = true;
                self.scan_char();
                if self.ch == u16::from(b'+') || self.ch == u16::from(b'-') {
                    self.scan_char();
                }
                while CharTypes::is_digit(self.ch) {
                    self.scan_char();
                }
            }
        }
        self.buf_pos = self.pos - self.mark;
        let token = if self.number_scale > 0 || self.number_exp {
            Token::LiteralFloat
        } else {
            Token::LiteralInt
        };
        self.token = Some(token);
        token
    }

    fn scan_variable(&mut self) {
        self.mark = self.pos;
        let prefix = self.ch;
        self.scan_char();
        if self.ch == prefix && (prefix == u16::from(b'@') || prefix == u16::from(b'$')) {
            self.scan_char();
        }
        if self.ch == u16::from(b'{') {
            self.scan_char();
            while !self.is_eof() && self.ch != u16::from(b'}') {
                self.scan_char();
            }
            if self.ch == u16::from(b'}') {
                self.scan_char();
            }
        } else {
            while CharTypes::is_identifier_char(self.ch) || CharTypes::is_digit(self.ch) {
                self.scan_char();
            }
        }
        self.buf_pos = self.pos - self.mark;
        self.string_val = Some(self.slice(self.mark, self.buf_pos));
        self.hash = fnv_units(self.token_slice(), false);
        self.hash_l_case = fnv_units(self.token_slice(), true);
        self.token = Some(Token::Variant);
    }

    fn scan_comment(&mut self) -> Result<Token, LexerError> {
        if !self.allow_comment {
            return Err(NotAllowCommentException::new().into());
        }
        let last_token = self.token;
        self.mark = self.pos;
        self.end_of_comment = false;
        let token;
        if self.ch == u16::from(b'/') && self.char_at(self.pos + 1) == u16::from(b'*') {
            token = Token::MultiLineComment;
            self.scan_char();
            self.scan_char();
            let mut depth = 1usize;
            while depth > 0 {
                if self.is_eof() {
                    return Err(ParserException::with_message(format!(
                        "unterminated /* comment. {}",
                        self.info()
                    ))
                    .into());
                }
                if self.ch == u16::from(b'/') && self.char_at(self.pos + 1) == u16::from(b'*') {
                    depth += 1;
                    self.scan_char();
                    self.scan_char();
                } else if self.ch == u16::from(b'*')
                    && self.char_at(self.pos + 1) == u16::from(b'/')
                {
                    depth -= 1;
                    self.scan_char();
                    self.scan_char();
                } else {
                    if self.ch == u16::from(b'\n') {
                        self.line += 1;
                    }
                    self.scan_char();
                }
            }
            self.end_of_comment = true;
        } else {
            token = Token::LineComment;
            self.scan_char();
            self.scan_char();
            while !self.is_eof() && self.ch != u16::from(b'\r') && self.ch != u16::from(b'\n') {
                self.scan_char();
            }
            if self.ch == u16::from(b'\r') {
                self.scan_char();
                if self.ch == u16::from(b'\n') {
                    self.scan_char();
                }
                self.line += 1;
            } else if self.ch == u16::from(b'\n') {
                self.scan_char();
                self.line += 1;
            }
            self.end_of_comment = true;
        }
        self.finish_comment(last_token, token);
        Ok(token)
    }

    fn finish_comment(&mut self, last_token: Option<Token>, token: Token) {
        self.buf_pos = self.pos - self.mark;
        let comment = self.slice(self.mark, self.buf_pos);
        self.string_val = Some(comment.clone());
        self.token = Some(token);
        self.comment_count += 1;
        if self.keep_comments {
            self.comments
                .get_or_insert_with(Vec::new)
                .push(comment.clone());
        }
        if let Some(handler) = &self.comment_handler {
            handler.handle(last_token, &comment);
        }
    }

    fn scan_operator(&mut self) -> Option<Token> {
        // 与 Java Token.name 对齐，必须按最长文本优先。
        const OPERATORS: &[(&str, Token)] = &[
            ("!~*", Token::BangTildeStar),
            ("??|", Token::Quesquesbar),
            ("||/", Token::Barbarslash),
            ("<=>", Token::Lteqgt),
            ("<->", Token::LtSubGt),
            ("<<<", Token::Ltltlt),
            (">>>", Token::Gtgtgt),
            ("->>", Token::Subgtgt),
            ("#>>", Token::Poundgtgt),
            ("...", Token::Dotdotdot),
            ("!!", Token::Bangbang),
            ("!~", Token::BangTilde),
            ("~*", Token::TildeStar),
            ("~=", Token::TildeEq),
            ("??", Token::Quesques),
            ("?|", Token::Quesbar),
            ("?&", Token::Quesamp),
            ("::", Token::Coloncolon),
            (":=", Token::Coloneq),
            ("==", Token::Eqeq),
            ("=>", Token::Eqgt),
            ("<=", Token::Lteq),
            ("<>", Token::Ltgt),
            (">=", Token::Gteq),
            ("!=", Token::Bangeq),
            ("!>", Token::Banggt),
            ("!<", Token::Banglt),
            ("&&", Token::Ampamp),
            ("||", Token::Barbar),
            ("|/", Token::Barslash),
            ("^=", Token::Careteq),
            ("->", Token::Subgt),
            ("<<", Token::Ltlt),
            (">>", Token::Gtgt),
            ("@@", Token::MonkeysAtAt),
            ("#>", Token::Poundgt),
            ("@>", Token::MonkeysAtGt),
            ("<@", Token::LtMonkeysAt),
            ("..", Token::Dotdot),
            ("(", Token::Lparen),
            ("（", Token::Lparen),
            (")", Token::Rparen),
            ("）", Token::Rparen),
            ("{", Token::Lbrace),
            ("}", Token::Rbrace),
            ("[", Token::Lbracket),
            ("]", Token::Rbracket),
            (";", Token::Semi),
            (",", Token::Comma),
            ("，", Token::Comma),
            (".", Token::Dot),
            ("=", Token::Eq),
            (">", Token::Gt),
            ("<", Token::Lt),
            ("!", Token::Bang),
            ("~", Token::Tilde),
            ("?", Token::Ques),
            (":", Token::Colon),
            ("+", Token::Plus),
            ("-", Token::Sub),
            ("*", Token::Star),
            ("/", Token::Slash),
            ("&", Token::Amp),
            ("|", Token::Bar),
            ("^", Token::Caret),
            ("%", Token::Percent),
            ("@", Token::MonkeysAt),
            ("#", Token::Pound),
        ];
        for &(text, token) in OPERATORS {
            if self.starts_with(text) {
                let start = self.pos;
                let length = text.encode_utf16().count();
                self.pos += length;
                self.ch = self.char_at(self.pos);
                return Some(self.finish_simple(token, start, length));
            }
        }
        None
    }

    fn finish_simple(&mut self, token: Token, mark: usize, length: usize) -> Token {
        self.mark = mark;
        self.buf_pos = length;
        self.token = Some(token);
        token
    }

    fn starts_with(&self, value: &str) -> bool {
        let candidate = value.encode_utf16().collect::<Vec<_>>();
        self.text
            .as_utf16()
            .get(self.pos..self.pos.saturating_add(candidate.len()))
            == Some(candidate.as_slice())
    }

    fn scan_char(&mut self) {
        self.pos += 1;
        self.ch = self.char_at(self.pos);
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.text.len()
    }

    fn slice(&self, offset: usize, count: usize) -> JavaString {
        JavaString::from_utf16(self.text.as_utf16()[offset..offset + count].to_vec())
    }

    fn token_slice(&self) -> &[u16] {
        &self.text.as_utf16()[self.mark..self.mark + self.buf_pos]
    }

    fn compute_row_and_column(&mut self) {
        let upto = self.pos.min(self.text.len());
        let mut line = 1usize;
        let mut column = 1usize;
        for unit in &self.text.as_utf16()[..upto] {
            if *unit == u16::from(b'\n') {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        self.pos_line = line;
        self.pos_column = column;
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
