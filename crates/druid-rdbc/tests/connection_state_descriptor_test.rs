use druid_rdbc::statement::{ResultSetHoldability, StatementOptions};

/// 标准已知常量保持不变，原驱动数字描述符可无损携带不属于常量的值。
#[test]
fn driver_holdability_numeric_identity_is_preserved() {
    assert_eq!(ResultSetHoldability::HoldOverCommit.rdbc_code(), 1);
    assert_eq!(ResultSetHoldability::CloseAtCommit.rdbc_code(), 2);
    assert_eq!(
        ResultSetHoldability::default(),
        ResultSetHoldability::CloseAtCommit
    );
    for code in [i32::MIN, -1, 0, 3, 9, i32::MAX] {
        let holdability = ResultSetHoldability::DriverValue(code);
        assert_eq!(holdability.rdbc_code(), code);
        let options = StatementOptions::new(Default::default(), Default::default(), holdability);
        assert_eq!(options.holdability().rdbc_code(), code);
    }
}
