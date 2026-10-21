use super::*;

impl Lexer {
    /// 从 Rust UTF-8 SQL 创建默认 Lexer。
    #[must_use]
    pub fn new(input: impl AsRef<str>) -> Self {
        Self::from_rdbc_string(RdbcString::from_rust_str(input.as_ref()), None, true)
    }

    /// 从 SQL 与数据库类型创建 Lexer。
    #[must_use]
    pub fn with_db_type(input: impl AsRef<str>, db_type: DbType) -> Self {
        Self::from_rdbc_string(
            RdbcString::from_rust_str(input.as_ref()),
            Some(db_type),
            true,
        )
    }

    /// 从无损 Java UTF-16 SQL 创建 Lexer。
    #[must_use]
    pub fn from_rdbc_string(text: RdbcString, db_type: Option<DbType>, skip_comment: bool) -> Self {
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
    pub const fn source(&self) -> &RdbcString {
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
    pub const fn string_val(&self) -> Option<&RdbcString> {
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
    pub fn comments(&self) -> Option<&[RdbcString]> {
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

    /// 配置 Java `SQLParserFeature` mask 及关联快捷字段。
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

    /// 判断 `SQLParserFeature` 是否启用。
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

            if (self.ch == u16::from(b'N') || self.ch == u16::from(b'n'))
                && self.char_at(self.pos + 1) == u16::from(b'\'')
            {
                self.pos += 1;
                self.ch = u16::from(b'\'');
                self.scan_string(Token::LiteralNchars)?;
                return Ok(Token::LiteralNchars);
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
                format!("illegal.char, {illegal}"),
                self.line as i32,
                self.pos_column as i32,
            )
            .into());
        }
    }
}
