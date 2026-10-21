use super::*;

impl Lexer {
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

    pub(crate) fn scan_identifier(&mut self) {
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
            .get_keyword_rdbc_string(&value)
            .or(Some(Token::Identifier));
        if self.token == Some(Token::Identifier) {
            self.string_val = Some(value);
        }
    }

    pub(crate) fn scan_quoted_identifier(
        &mut self,
        quote: u16,
        token: Token,
    ) -> Result<(), ParserException> {
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
            RdbcString::from_utf16(value)
        });
        self.token = Some(token);
        Ok(())
    }

    pub(crate) fn scan_string(&mut self, token: Token) -> Result<(), ParserException> {
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
        self.string_val = Some(RdbcString::from_utf16(value));
        self.token = Some(token);
        Ok(())
    }

    pub(crate) fn scan_number(&mut self) -> Token {
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

    pub(crate) fn scan_variable(&mut self) {
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

    pub(crate) fn scan_comment(&mut self) -> Result<Token, LexerError> {
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

    pub(crate) fn finish_comment(&mut self, last_token: Option<Token>, token: Token) {
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

    pub(crate) fn scan_operator(&mut self) -> Option<Token> {
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

    pub(crate) fn finish_simple(&mut self, token: Token, mark: usize, length: usize) -> Token {
        self.mark = mark;
        self.buf_pos = length;
        self.token = Some(token);
        token
    }

    pub(crate) fn starts_with(&self, value: &str) -> bool {
        let candidate = value.encode_utf16().collect::<Vec<_>>();
        self.text
            .as_utf16()
            .get(self.pos..self.pos.saturating_add(candidate.len()))
            == Some(candidate.as_slice())
    }

    pub(crate) fn scan_char(&mut self) {
        self.pos += 1;
        self.ch = self.char_at(self.pos);
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.pos >= self.text.len()
    }

    pub(crate) fn slice(&self, offset: usize, count: usize) -> RdbcString {
        RdbcString::from_utf16(self.text.as_utf16()[offset..offset + count].to_vec())
    }

    pub(crate) fn token_slice(&self) -> &[u16] {
        &self.text.as_utf16()[self.mark..self.mark + self.buf_pos]
    }

    pub(crate) fn compute_row_and_column(&mut self) {
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
