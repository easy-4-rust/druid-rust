//! 可空游标入口的第三方最小实现兼容性及 NULL 保真合同。

use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::Statement;

/// 既有外部实现，只实现旧非空入口，不能把 NULL 当成空字符串。
#[derive(Default)]
struct LegacyStatement {
    names: Vec<String>,
}
druid_rdbc::impl_standard_wrapper!(LegacyStatement => Statement);

#[async_trait::async_trait]
impl Statement for LegacyStatement {
    fn set_cursor_name(&mut self, name: &str) -> Result<(), RdbcError> {
        self.names.push(name.to_owned());
        Ok(())
    }
}

#[test]
fn nullable_default_preserves_existing_implementations_and_never_erases_null() {
    let mut statement = LegacyStatement::default();
    statement.set_cursor_name_nullable(Some("cursor")).unwrap();
    statement.set_cursor_name_nullable(Some("")).unwrap();
    let error = statement.set_cursor_name_nullable(None).unwrap_err();
    assert_eq!(
        error.sql_exception().unwrap().class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("0A000"));
    assert_eq!(statement.names, vec!["cursor", ""]);
}
