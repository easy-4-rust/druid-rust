//! SQL 指纹（P3.3a）：从 `druid::stats::merge` 下沉，供 Wall 统计复用。
//!
//! 对应 Java: `com.alibaba.druid.stat.DruidStatManagerFacade` 对 SQL 归一化后的哈希职责。

/// SQL 模板指纹。
#[must_use]
pub fn fingerprint(sql_template: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(sql_template.as_bytes())
}
