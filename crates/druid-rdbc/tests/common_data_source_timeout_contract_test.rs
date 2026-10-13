use druid_rdbc::data_source::CommonDataSource;
use druid_rdbc::error::RdbcError;
use std::time::Duration;

struct LegacyDataSource(Option<Duration>);

impl CommonDataSource for LegacyDataSource {
    fn login_timeout(&self) -> Option<Duration> {
        self.0
    }

    fn set_login_timeout(&mut self, timeout: Duration) -> Result<(), RdbcError> {
        self.0 = Some(timeout);
        Ok(())
    }
}

struct FailingDataSource;

impl CommonDataSource for FailingDataSource {
    fn try_login_timeout(&self) -> Result<Option<Duration>, RdbcError> {
        Err(RdbcError::LoginTimeout)
    }

    fn set_login_timeout(&mut self, _timeout: Duration) -> Result<(), RdbcError> {
        Err(RdbcError::InvalidArgument(
            "timeout rejected by source".to_owned(),
        ))
    }
}

#[test]
fn fallible_timeout_getter_preserves_default_zero_and_duration_boundaries() {
    let default_source = LegacyDataSource(None);
    assert_eq!(default_source.try_login_timeout().unwrap(), None);

    let mut zero_source = LegacyDataSource(None);
    zero_source.set_login_timeout(Duration::ZERO).unwrap();
    assert_eq!(
        zero_source.try_login_timeout().unwrap(),
        Some(Duration::ZERO)
    );

    let maximum = Duration::new(u64::MAX, 999_999_999);
    let maximum_source = LegacyDataSource(Some(maximum));
    assert_eq!(maximum_source.try_login_timeout().unwrap(), Some(maximum));
}

#[test]
fn timeout_read_and_write_errors_preserve_their_original_payloads() {
    let mut source = FailingDataSource;
    assert!(matches!(
        source.try_login_timeout(),
        Err(RdbcError::LoginTimeout)
    ));
    assert!(matches!(
        source.set_login_timeout(Duration::from_secs(7)),
        Err(RdbcError::InvalidArgument(message)) if message == "timeout rejected by source"
    ));
}
