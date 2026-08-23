use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryPackage {
    path: PathBuf,
}

impl TemporaryPackage {
    fn create() -> Result<Self, Box<dyn Error>> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "druid-rdbc-external-consumer-{}-{elapsed}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryPackage {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.path);
    }
}

fn toml_path(path: &Path) -> Result<String, Box<dyn Error>> {
    let path = path
        .to_str()
        .ok_or("druid package path is not valid UTF-8")?;
    Ok(path.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 验证独立包仍可通过既有 druid::sql::* 公共路径编译。
#[test]
fn external_consumer_retains_druid_sql_public_contract() -> Result<(), Box<dyn Error>> {
    let package = TemporaryPackage::create()?;
    let druid_path = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    let druid_rdbc_path = druid_path
        .parent()
        .ok_or("druid package has no workspace crates directory")?
        .join("druid-rdbc")
        .canonicalize()?;
    let manifest = format!(
        "[package]\nname = \"rdbc-external-consumer-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\ndruid = {{ path = \"{}\" }}\ndruid-rdbc = {{ path = \"{}\" }}\n",
        toml_path(&druid_path)?,
        toml_path(&druid_rdbc_path)?,
    );
    let source = r"use druid::sql::{Connection, DataSource, Date, ResultSet, SQLException, Statement};

macro_rules! bidirectional_owned_identity {
    ($old:path, $new:path) => {{
        let old_value: Option<$old> = None;
        let new_value: Option<$new> = old_value;
        let _old_value: Option<$old> = new_value;
    }};
}

macro_rules! bidirectional_dyn_identity {
    ($old:path, $new:path) => {{
        fn old_to_new(value: &dyn $old) -> &dyn $new {
            value
        }
        fn new_to_old(value: &dyn $new) -> &dyn $old {
            value
        }
        let _ = old_to_new;
        let _ = new_to_old;
    }};
}

fn accepts_connection(_: Option<Connection>) {}
fn accepts_result_set(_: Option<ResultSet>) {}

fn main() {
    accepts_connection(None);
    accepts_result_set(None);
    let _ = std::any::TypeId::of::<Date>();

    bidirectional_owned_identity!(druid::core::value::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::Value, druid_rdbc::types::Value);
    bidirectional_owned_identity!(druid::core::row::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::connection::Row, druid_rdbc::types::Row);
    bidirectional_owned_identity!(druid::core::rdbc_url::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::core::RdbcUrl, druid_rdbc::types::RdbcUrl);
    bidirectional_owned_identity!(druid::sql::types::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::Types, druid_rdbc::types::Types);
    bidirectional_owned_identity!(druid::sql::rdbc_type::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RdbcType, druid_rdbc::types::RdbcType);
    bidirectional_owned_identity!(druid::sql::RDBCType, druid_rdbc::types::RdbcType);
    bidirectional_dyn_identity!(druid::sql::rdbc_sql_type::SqlType, druid_rdbc::types::SqlType);
    bidirectional_dyn_identity!(druid::sql::SQLType, druid_rdbc::types::SqlType);

    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlExceptionCause,
        druid_rdbc::error::SqlExceptionCause
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionCause, druid_rdbc::error::SqlExceptionCause);
    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::core::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlExceptionKind,
        druid_rdbc::error::SqlExceptionKind
    );
    bidirectional_owned_identity!(druid::sql::SqlExceptionKind, druid_rdbc::error::SqlExceptionKind);

    bidirectional_owned_identity!(
        druid::core::sql_exception::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::core::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(druid::sql::SQLException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTransientException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlNonTransientConnectionException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlRecoverableException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlRecoverableException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlDataException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlDataException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlIntegrityConstraintViolationException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlInvalidAuthorizationSpecException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlSyntaxErrorException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlSyntaxErrorException, druid_rdbc::error::SqlException);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlTransactionRollbackException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::SqlFeatureNotSupportedException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlTimeoutException,
        druid_rdbc::error::SqlException
    );
    bidirectional_owned_identity!(druid::sql::SqlTimeoutException, druid_rdbc::error::SqlException);

    bidirectional_owned_identity!(
        druid::core::sql_warning::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::core::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(
        druid::sql::exceptions::SqlWarning,
        druid_rdbc::error::SqlWarning
    );
    bidirectional_owned_identity!(druid::sql::SqlWarning, druid_rdbc::error::SqlWarning);
    bidirectional_owned_identity!(druid::sql::SQLWarning, druid_rdbc::error::SqlWarning);

    mod sql_glob_contract {
        use druid::sql::*;

        pub fn proves_glob_alias_identity(
            exception: Option<SQLException>,
            warning: Option<SQLWarning>,
            rdbc_type: Option<RDBCType>,
            types: Option<Types>,
        ) -> (
            Option<druid_rdbc::error::SqlException>,
            Option<druid_rdbc::error::SqlWarning>,
            Option<druid_rdbc::types::RdbcType>,
            Option<druid_rdbc::types::Types>,
        ) {
            (exception, warning, rdbc_type, types)
        }
    }
    let _ = sql_glob_contract::proves_glob_alias_identity(None, None, None, None);
}
";
    fs::write(package.path.join("Cargo.toml"), manifest)?;
    fs::create_dir(package.path.join("src"))?;
    fs::write(package.path.join("src/main.rs"), source)?;

    let output = Command::new(env!("CARGO"))
        .arg("check")
        .arg("--quiet")
        .current_dir(&package.path)
        .env("CARGO_TARGET_DIR", package.path.join("target"))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "external consumer cargo check failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
        .into());
    }

    Ok(())
}

/// 验证结构化 SQL 异常与警告在旧、新路径之间保持同一类型和完整字段语义。
#[test]
fn error_identity() {
    let mut exception = druid_rdbc::error::SqlException::new(
        1205,
        Some("40001".to_string()),
        Some("deadlock victim".to_string()),
    )
    .with_class_name("vendor.DeadlockException")
    .with_assignable_type("vendor.DatabaseException")
    .with_cause(druid_rdbc::error::SqlExceptionCause::ClassName(
        "vendor.RootCause".to_string(),
    ));
    exception.set_next_exception(druid_rdbc::error::SqlException::driver(
        1213,
        "retry transaction",
    ));

    let old_leaf: druid::core::sql_exception::SqlException = exception;
    let old_parent: druid::core::SqlException = old_leaf;
    let old_sql_alias: druid::sql::SQLException = old_parent;
    let canonical: druid_rdbc::error::SqlException = old_sql_alias;

    assert_eq!(canonical.error_code(), 1205);
    assert_eq!(canonical.sql_state(), Some("40001"));
    assert_eq!(canonical.message(), Some("deadlock victim"));
    assert_eq!(canonical.class_name(), "vendor.DeadlockException");
    assert!(canonical.is_instance_of("vendor.DatabaseException"));
    assert_eq!(
        canonical.causes(),
        &[druid_rdbc::error::SqlExceptionCause::ClassName(
            "vendor.RootCause".to_string(),
        )]
    );
    assert_eq!(
        canonical
            .next_exception()
            .and_then(druid_rdbc::error::SqlException::message),
        Some("retry transaction")
    );

    let mut warning =
        druid_rdbc::error::SqlWarning::new("plan fallback", Some("01000".to_string()), 7);
    warning.set_next_warning(druid_rdbc::error::SqlWarning::new(
        "second warning",
        None,
        8,
    ));
    let old_warning: druid::core::sql_warning::SqlWarning = warning;
    let old_sql_warning: druid::sql::SQLWarning = old_warning;
    let canonical_warning: druid_rdbc::error::SqlWarning = old_sql_warning;

    assert_eq!(canonical_warning.message(), "plan fallback");
    assert_eq!(canonical_warning.sql_state(), Some("01000"));
    assert_eq!(canonical_warning.error_code(), 7);
    assert_eq!(
        canonical_warning
            .next_warning()
            .map(druid_rdbc::error::SqlWarning::message),
        Some("second warning")
    );
}
