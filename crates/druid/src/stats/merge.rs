//! 对应 Java 类：com.alibaba.druid.stat.JdbcSqlStat + SqlMerger
//!
//! SQL 合并统计：把参数化后的 SQL 模板作为 key，聚合执行统计。

use std::time::Duration;

use super::JdbcSqlStat;

/// 旧内部名称，保留源码兼容；canonical 对象为 [`JdbcSqlStat`]。
pub type MergedSqlStat = JdbcSqlStat;

/// SQL 指纹（xxh3 哈希）。
pub fn fingerprint(sql_template: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(sql_template.as_bytes())
}

/// 参数化 SQL 结果。
#[derive(Debug, Clone)]
pub struct ParameterizedSql {
    pub template: String,
    pub fingerprint: u64,
}

/// SQL 词法参数化：只把字符串和数值字面量替换为 `?`。
///
/// 对应 Java `SQLUtils#parameterize` 的统计合并职责。标识符、已有 placeholder、
/// quoted identifier 与注释保持不变；字符串转义和科学计数法作为一个字面量
/// 消费，避免旧实现把 `table1` 错误改成 `table?`。
pub fn parameterize(sql: &str) -> ParameterizedSql {
    let mut template = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                if index > 0
                    && matches!(
                        bytes[index - 1],
                        b'n' | b'N' | b'e' | b'E' | b'x' | b'X' | b'b' | b'B'
                    )
                    && is_token_start(bytes, index - 1)
                {
                    template.pop();
                }
                index = consume_quoted(bytes, index, b'\'');
                template.push('?');
            }
            b'"' | b'`' => {
                let end = consume_quoted(bytes, index, bytes[index]);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'[' => {
                let end = consume_bracket_identifier(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                let end = consume_line_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'#' => {
                let end = consume_line_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = consume_block_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            byte if byte.is_ascii_digit() && is_token_start(bytes, index) => {
                index = consume_number(bytes, index);
                template.push('?');
            }
            b'.' if bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
                && is_token_start(bytes, index) =>
            {
                index = consume_number(bytes, index);
                template.push('?');
            }
            _ => {
                let ch = sql[index..]
                    .chars()
                    .next()
                    .expect("index is on a UTF-8 boundary");
                template.push(ch);
                index += ch.len_utf8();
            }
        }
    }

    let fp = fingerprint(&template);
    ParameterizedSql {
        template,
        fingerprint: fp,
    }
}

fn consume_quoted(bytes: &[u8], mut index: usize, quote: u8) -> usize {
    index += 1;
    while index < bytes.len() {
        if bytes[index] == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return index + 1;
        }
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn consume_bracket_identifier(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b']' {
            if bytes.get(index + 1) == Some(&b']') {
                index += 2;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn consume_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
        index += 1;
    }
    index
}

fn consume_block_comment(bytes: &[u8], mut index: usize) -> usize {
    index += 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn is_token_start(bytes: &[u8], index: usize) -> bool {
    index == 0
        || bytes[index - 1].is_ascii()
            && !bytes[index - 1].is_ascii_alphanumeric()
            && !matches!(bytes[index - 1], b'_' | b'$')
}

fn consume_number(bytes: &[u8], mut index: usize) -> usize {
    if bytes[index] == b'.' {
        index += 1;
    }
    if bytes.get(index) == Some(&b'0')
        && bytes
            .get(index + 1)
            .is_some_and(|byte| matches!(byte, b'x' | b'X' | b'b' | b'B'))
    {
        index += 2;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'_')
        {
            index += 1;
        }
        return index;
    }
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
    {
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            index += 1;
        }
    }
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        let exponent = index;
        index += 1;
        if bytes
            .get(index)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            index += 1;
        }
        let digits = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if digits == index {
            return exponent;
        }
    }
    index
}

/// SQL 合并器。
///
/// 对应 Druid Java 的 `DruidStatService` 中的 SQL 合并逻辑。
pub struct SqlMerger {
    cache: dashmap::DashMap<u64, std::sync::Arc<JdbcSqlStat>>,
}

impl SqlMerger {
    pub fn new() -> Self {
        Self {
            cache: dashmap::DashMap::new(),
        }
    }

    /// 记录一条 SQL 执行。
    pub fn record(&self, sql: &str, elapsed: Duration, ok: bool) {
        self.record_with_merge(sql, elapsed, ok, true);
    }

    /// 按 `StatFilter.mergeSql` 配置记录 SQL。
    ///
    /// `merge_sql=false` 时使用原始 SQL 文本及其哈希作为统计键；开启时才执行
    /// Druid 参数化合并。对应 Java：
    /// `StatFilter#createSqlStat(StatementProxy, String)`。
    pub fn record_with_merge(&self, sql: &str, elapsed: Duration, ok: bool, merge_sql: bool) {
        let param = if merge_sql {
            parameterize(sql)
        } else {
            ParameterizedSql {
                template: sql.to_owned(),
                fingerprint: fingerprint(sql),
            }
        };
        let stat = self
            .cache
            .entry(param.fingerprint)
            .or_insert_with(|| {
                std::sync::Arc::new(JdbcSqlStat::new(param.template, param.fingerprint))
            })
            .clone();
        stat.record(elapsed, ok);
    }

    /// 获取所有 SQL 统计。
    pub fn all_stats(&self) -> Vec<std::sync::Arc<JdbcSqlStat>> {
        self.cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 获取指定指纹的统计。
    pub fn get_stat(&self, fingerprint: u64) -> Option<std::sync::Arc<JdbcSqlStat>> {
        self.cache.get(&fingerprint).map(|v| v.clone())
    }

    /// SQL 模板数量。
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 清空全部 SQL 聚合项。
    pub fn reset(&self) {
        self.cache.clear();
    }
}

impl Default for SqlMerger {
    fn default() -> Self {
        Self::new()
    }
}
