use super::PreparedStatementKey;
use crate::core::prepared_statement_method_type::PreparedStatementMethodType;
use druid_rdbc::statement::{GeneratedKeys, ResultSetHoldability, StatementOptions};

/// 缓存键保留每个真实驱动 holdability 原值，不能把不同值合并成0。
#[test]
fn holdability_cache_keys_preserve_driver_numeric_identity() {
    let codes = [i32::MIN, -1, 0, 3, 9, i32::MAX];
    let mut keys = Vec::new();
    for code in codes {
        let options = StatementOptions::new(
            Default::default(),
            Default::default(),
            ResultSetHoldability::DriverValue(code),
        );
        let key = PreparedStatementKey::standard(
            "select 1",
            Some("catalog_a".to_owned()),
            PreparedStatementMethodType::M3,
            options,
            GeneratedKeys::None,
        )
        .unwrap();
        assert_eq!(key.result_set_holdability(), code);
        assert!(keys.iter().all(|other| other != &key));
        keys.push(key);
    }
}

/// 不同游标类型及并发模式不得误命中同一缓存项。
#[test]
fn result_set_driver_codes_preserve_prepared_cache_identity() {
    use druid_rdbc::statement::{FetchDirection, ResultSetConcurrency, ResultSetType};
    let mut keys = Vec::new();
    for code in [i32::MIN, -1, 0, 2004, i32::MAX] {
        let options = StatementOptions::new(
            ResultSetType::DriverValue(code),
            ResultSetConcurrency::DriverValue(code),
            ResultSetHoldability::HoldOverCommit,
        );
        let key = PreparedStatementKey::standard(
            "select 1",
            None,
            PreparedStatementMethodType::M3,
            options,
            GeneratedKeys::None,
        )
        .unwrap();
        assert_eq!(key.result_set_type(), code);
        assert_eq!(key.result_set_concurrency(), code);
        assert!(keys.iter().all(|other| other != &key));
        keys.push(key);
    }
    assert!(FetchDirection::try_from(2000).is_err());
    assert_eq!(FetchDirection::DriverValue(2000).rdbc_code(), 2000);
}
