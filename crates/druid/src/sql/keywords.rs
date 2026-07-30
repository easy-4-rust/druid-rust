//! Druid SQL lexer 关键字表。

use crate::core::JavaString;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::Token;

const FNV_BASIC: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 默认 Druid 关键字表。
pub static DEFAULT_KEYWORDS: LazyLock<Keywords> = LazyLock::new(Keywords::default_keywords);

/// SQLite 关键字表。
///
/// Java baseline 在默认表基础上再次写入同一个 LIMIT 映射，最终内容相同但
/// 保持独立对象身份。
pub static SQLITE_KEYWORDS: LazyLock<Keywords> = LazyLock::new(|| {
    let mut keywords = DEFAULT_KEYWORDS.keywords.clone();
    keywords.insert("LIMIT".to_owned(), Token::Limit);
    Keywords::new(keywords)
});

/// 达梦关键字表。
pub static DM_KEYWORDS: LazyLock<Keywords> = LazyLock::new(|| {
    let mut keywords = DEFAULT_KEYWORDS.keywords.clone();
    keywords.insert("MERGE".to_owned(), Token::Merge);
    keywords.insert("MATCHED".to_owned(), Token::Matched);
    keywords.insert("USING".to_owned(), Token::Using);
    Keywords::new(keywords)
});

/// Druid lexer 关键字集合。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.Keywords`。原始 map、按 Java
/// UTF-16 ASCII-lower FNV-1a 计算的有符号 long hash 数组和对应 Token 数组
/// 同时保留；查询不使用 Rust Unicode lowercase。
#[derive(Debug, Clone)]
pub struct Keywords {
    keywords: HashMap<String, Token>,
    hash_array: Vec<i64>,
    tokens: Vec<Token>,
}

impl Keywords {
    /// 从关键字 map 创建二分查找表。
    ///
    /// Java 对 FNV hash 碰撞没有二次字符串比较，后遍历项覆盖同一槽位；
    /// Rust 保持同一算法。内置关键字不存在碰撞。
    #[must_use]
    pub fn new(keywords: HashMap<String, Token>) -> Self {
        let mut hash_array = keywords
            .keys()
            .map(|key| Self::fnv1a_64_lower(key))
            .collect::<Vec<_>>();
        hash_array.sort_unstable();

        let mut tokens = vec![Token::Error; hash_array.len()];
        for (key, token) in &keywords {
            let hash = Self::fnv1a_64_lower(key);
            if let Ok(index) = hash_array.binary_search(&hash) {
                tokens[index] = *token;
            }
        }

        Self {
            keywords,
            hash_array,
            tokens,
        }
    }

    /// 返回集合是否包含指定 Token 值。
    #[must_use]
    pub fn contains_value(&self, token: Token) -> bool {
        self.keywords.values().any(|candidate| *candidate == token)
    }

    /// 按 Java 有符号 long FNV hash 查询关键字。
    #[must_use]
    pub fn get_keyword_by_hash(&self, hash: i64) -> Option<Token> {
        self.hash_array
            .binary_search(&hash)
            .ok()
            .map(|index| self.tokens[index])
    }

    /// 按 Rust 字符串查询关键字；ASCII 大小写不敏感。
    #[must_use]
    pub fn get_keyword(&self, key: &str) -> Option<Token> {
        self.get_keyword_by_hash(Self::fnv1a_64_lower(key))
    }

    /// 按无损 Java UTF-16 字符串查询关键字。
    #[must_use]
    pub fn get_keyword_java_string(&self, key: &JavaString) -> Option<Token> {
        self.get_keyword_by_hash(Self::fnv1a_64_lower_java_string(key))
    }

    /// 返回构造时保存的原始关键字 map。
    #[must_use]
    pub const fn get_keywords(&self) -> &HashMap<String, Token> {
        &self.keywords
    }

    /// 计算 Java `FnvHash.fnv1a_64_lower(String)`。
    #[must_use]
    pub fn fnv1a_64_lower(key: &str) -> i64 {
        Self::fnv1a_64_lower_units(key.encode_utf16())
    }

    /// 对无损 Java String 计算相同 hash。
    #[must_use]
    pub fn fnv1a_64_lower_java_string(key: &JavaString) -> i64 {
        Self::fnv1a_64_lower_units(key.as_utf16().iter().copied())
    }

    fn fnv1a_64_lower_units(units: impl IntoIterator<Item = u16>) -> i64 {
        let mut hash = FNV_BASIC;
        for mut unit in units {
            if unit >= u16::from(b'A') && unit <= u16::from(b'Z') {
                unit += 32;
            }
            hash ^= u64::from(unit);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash as i64
    }

    fn default_keywords() -> Self {
        let mut map = HashMap::new();
        map.insert("ALL".to_owned(), Token::All);
        map.insert("ALTER".to_owned(), Token::Alter);
        map.insert("AND".to_owned(), Token::And);
        map.insert("ANY".to_owned(), Token::Any);
        map.insert("AS".to_owned(), Token::As);
        map.insert("ENABLE".to_owned(), Token::Enable);
        map.insert("DISABLE".to_owned(), Token::Disable);
        map.insert("ASC".to_owned(), Token::Asc);
        map.insert("BETWEEN".to_owned(), Token::Between);
        map.insert("BY".to_owned(), Token::By);
        map.insert("CASE".to_owned(), Token::Case);
        map.insert("CAST".to_owned(), Token::Cast);
        map.insert("CHECK".to_owned(), Token::Check);
        map.insert("CONSTRAINT".to_owned(), Token::Constraint);
        map.insert("CREATE".to_owned(), Token::Create);
        map.insert("DATABASE".to_owned(), Token::Database);
        map.insert("DEFAULT".to_owned(), Token::Default);
        map.insert("DIAGNOSTICS".to_owned(), Token::Diagnostics);
        map.insert("COLUMN".to_owned(), Token::Column);
        map.insert("TABLESPACE".to_owned(), Token::Tablespace);
        map.insert("PROCEDURE".to_owned(), Token::Procedure);
        map.insert("FUNCTION".to_owned(), Token::Function);
        map.insert("DELETE".to_owned(), Token::Delete);
        map.insert("DESC".to_owned(), Token::Desc);
        map.insert("DISTINCT".to_owned(), Token::Distinct);
        map.insert("DROP".to_owned(), Token::Drop);
        map.insert("ELSE".to_owned(), Token::Else);
        map.insert("EXPLAIN".to_owned(), Token::Explain);
        map.insert("EXCEPT".to_owned(), Token::Except);
        map.insert("END".to_owned(), Token::End);
        map.insert("ESCAPE".to_owned(), Token::Escape);
        map.insert("EXISTS".to_owned(), Token::Exists);
        map.insert("FOR".to_owned(), Token::For);
        map.insert("FOREIGN".to_owned(), Token::Foreign);
        map.insert("FROM".to_owned(), Token::From);
        map.insert("FULL".to_owned(), Token::Full);
        map.insert("GET".to_owned(), Token::Get);
        map.insert("GROUP".to_owned(), Token::Group);
        map.insert("HAVING".to_owned(), Token::Having);
        map.insert("IN".to_owned(), Token::In);
        map.insert("INDEX".to_owned(), Token::Index);
        map.insert("INNER".to_owned(), Token::Inner);
        map.insert("INSERT".to_owned(), Token::Insert);
        map.insert("INTERSECT".to_owned(), Token::Intersect);
        map.insert("INTERVAL".to_owned(), Token::Interval);
        map.insert("INTO".to_owned(), Token::Into);
        map.insert("IS".to_owned(), Token::Is);
        map.insert("JOIN".to_owned(), Token::Join);
        map.insert("KEY".to_owned(), Token::Key);
        map.insert("LEFT".to_owned(), Token::Left);
        map.insert("LIKE".to_owned(), Token::Like);
        map.insert("LOCK".to_owned(), Token::Lock);
        map.insert("MINUS".to_owned(), Token::Minus);
        map.insert("NOT".to_owned(), Token::Not);
        map.insert("NULL".to_owned(), Token::Null);
        map.insert("ON".to_owned(), Token::On);
        map.insert("OR".to_owned(), Token::Or);
        map.insert("ORDER".to_owned(), Token::Order);
        map.insert("OUTER".to_owned(), Token::Outer);
        map.insert("PRIMARY".to_owned(), Token::Primary);
        map.insert("REFERENCES".to_owned(), Token::References);
        map.insert("RIGHT".to_owned(), Token::Right);
        map.insert("SCHEMA".to_owned(), Token::Schema);
        map.insert("SELECT".to_owned(), Token::Select);
        map.insert("SET".to_owned(), Token::Set);
        map.insert("SOME".to_owned(), Token::Some);
        map.insert("TABLE".to_owned(), Token::Table);
        map.insert("THEN".to_owned(), Token::Then);
        map.insert("TRUNCATE".to_owned(), Token::Truncate);
        map.insert("UNION".to_owned(), Token::Union);
        map.insert("UNIQUE".to_owned(), Token::Unique);
        map.insert("UPDATE".to_owned(), Token::Update);
        map.insert("VALUES".to_owned(), Token::Values);
        map.insert("VIEW".to_owned(), Token::View);
        map.insert("SEQUENCE".to_owned(), Token::Sequence);
        map.insert("TRIGGER".to_owned(), Token::Trigger);
        map.insert("USER".to_owned(), Token::User);
        map.insert("WHEN".to_owned(), Token::When);
        map.insert("WHERE".to_owned(), Token::Where);
        map.insert("XOR".to_owned(), Token::Xor);
        map.insert("OVER".to_owned(), Token::Over);
        map.insert("TO".to_owned(), Token::To);
        map.insert("USE".to_owned(), Token::Use);
        map.insert("REPLACE".to_owned(), Token::Replace);
        map.insert("COMMENT".to_owned(), Token::Comment);
        map.insert("COMPUTE".to_owned(), Token::Compute);
        map.insert("WITH".to_owned(), Token::With);
        map.insert("GRANT".to_owned(), Token::Grant);
        map.insert("REVOKE".to_owned(), Token::Revoke);
        map.insert("WHILE".to_owned(), Token::While);
        map.insert("DO".to_owned(), Token::Do);
        map.insert("DECLARE".to_owned(), Token::Declare);
        map.insert("LOOP".to_owned(), Token::Loop);
        map.insert("LEAVE".to_owned(), Token::Leave);
        map.insert("ITERATE".to_owned(), Token::Iterate);
        map.insert("REPEAT".to_owned(), Token::Repeat);
        map.insert("UNTIL".to_owned(), Token::Until);
        map.insert("OPEN".to_owned(), Token::Open);
        map.insert("CLOSE".to_owned(), Token::Close);
        map.insert("CURSOR".to_owned(), Token::Cursor);
        map.insert("FETCH".to_owned(), Token::Fetch);
        map.insert("OUT".to_owned(), Token::Out);
        map.insert("INOUT".to_owned(), Token::Inout);
        map.insert("LIMIT".to_owned(), Token::Limit);
        Self::new(map)
    }
}
