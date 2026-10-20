use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{CommonDataSource, DataSource};
use druid_rdbc::error::{RdbcError, SqlException};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Default)]
struct LegacySource {
    ordinary: AtomicUsize,
    supplied: Mutex<Vec<(String, String)>>,
}
impl CommonDataSource for LegacySource {}
druid_rdbc::impl_standard_wrapper!(LegacySource => DataSource);
#[async_trait::async_trait]
impl DataSource for LegacySource {
    async fn connection(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.ordinary.fetch_add(1, Ordering::AcqRel);
        Err(RdbcError::LoginTimeout)
    }
    async fn connection_with_credentials(
        &self,
        user: &str,
        password: &str,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        self.supplied
            .lock()
            .unwrap()
            .push((user.to_owned(), password.to_owned()));
        Err(RdbcError::SqlException(Box::new(SqlException::new(
            73,
            Some("28000".to_owned()),
            Some("verifier rejected".to_owned()),
        ))))
    }
}

#[tokio::test]
async fn legacy_default_preserves_null_empty_values_and_original_errors() {
    let source = LegacySource::default();
    let view: &dyn DataSource = &source;
    let error = view
        .connection_with_nullable_credentials(None, None)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, RdbcError::LoginTimeout));
    assert_eq!(source.ordinary.load(Ordering::Acquire), 1);
    for (user, password) in [("", ""), ("user", ""), ("", "secret"), ("user", "secret")] {
        let error = view
            .connection_with_nullable_credentials(Some(user), Some(password))
            .await
            .err()
            .unwrap();
        let exception = error.sql_exception().unwrap();
        assert_eq!(exception.error_code(), 73);
        assert_eq!(exception.sql_state(), Some("28000"));
        assert_eq!(exception.message(), Some("verifier rejected"));
    }
    assert_eq!(
        *source.supplied.lock().unwrap(),
        vec![
            ("".to_owned(), "".to_owned()),
            ("user".to_owned(), "".to_owned()),
            ("".to_owned(), "secret".to_owned()),
            ("user".to_owned(), "secret".to_owned())
        ]
    );
    for (user, password) in [
        (None, Some("")),
        (Some(""), None),
        (None, Some("secret")),
        (Some("user"), None),
    ] {
        let error = view
            .connection_with_nullable_credentials(user, password)
            .await
            .err()
            .unwrap();
        assert_eq!(error.sql_exception().unwrap().sql_state(), Some("0A000"));
    }
    assert_eq!(source.ordinary.load(Ordering::Acquire), 1);
    assert_eq!(source.supplied.lock().unwrap().len(), 4);
}

#[derive(Default)]
struct NullableSource(AtomicUsize);
impl CommonDataSource for NullableSource {}
druid_rdbc::impl_standard_wrapper!(NullableSource => DataSource);
#[async_trait::async_trait]
impl DataSource for NullableSource {
    async fn connection(&self) -> Result<Box<dyn Connection>, RdbcError> {
        panic!("nullable override must not be replaced by ordinary acquisition")
    }
    async fn connection_with_nullable_credentials(
        &self,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Box<dyn Connection>, RdbcError> {
        assert_eq!(user, None);
        assert_eq!(password, Some(""));
        self.0.fetch_add(1, Ordering::AcqRel);
        Err(RdbcError::LoginTimeout)
    }
}

#[tokio::test]
async fn nullable_override_dispatch_keeps_partial_null_and_empty_distinct() {
    let source = NullableSource::default();
    let view: &dyn DataSource = &source;
    let error = view
        .connection_with_nullable_credentials(None, Some(""))
        .await
        .err()
        .unwrap();
    assert!(matches!(error, RdbcError::LoginTimeout));
    assert_eq!(source.0.load(Ordering::Acquire), 1);
}
