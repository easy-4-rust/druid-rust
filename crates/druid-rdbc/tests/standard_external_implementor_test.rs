use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn external_crate_implements_standard_without_druid_dependency() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch for a unique test directory")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("druid-rdbc-external-{unique}"));
    fs::create_dir_all(root.join("src"))
        .expect("external implementor fixture directory must be created");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = format!(
        r#"[package]
name = "rdbc-external-implementor"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
async-trait = "0.1"
druid-rdbc = {{ path = {manifest_dir:?} }}
"#,
    );
    fs::write(root.join("Cargo.toml"), manifest)
        .expect("external implementor manifest must be written");
    fs::write(
        root.join("src/main.rs"),
        r"use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::wrapper::Wrapper;
use std::any::Any;

struct ExternalConnection;
struct ExternalStatement;
struct ExternalResultSet;

macro_rules! impl_wrapper {
    ($($target:ty),+ $(,)?) => {
        $(
            impl Wrapper for $target {
                fn as_any(&self) -> &dyn Any {
                    self
                }
            }
        )+
    };
}

impl_wrapper!(ExternalConnection, ExternalStatement, ExternalResultSet);

#[async_trait::async_trait]
impl Connection for ExternalConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(ExternalStatement))
    }
}

#[async_trait::async_trait]
impl Statement for ExternalStatement {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(ExternalResultSet))
    }
}

#[async_trait::async_trait]
impl ResultSet for ExternalResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

fn assert_send_static<T: Send + 'static + ?Sized>() {}

fn main() {
    let _: Option<Box<dyn Connection>> = None;
    let _: Option<Box<dyn Statement>> = None;
    let _: Option<Box<dyn ResultSet>> = None;
    assert_send_static::<dyn Connection>();
    assert_send_static::<dyn Statement>();
    assert_send_static::<dyn ResultSet>();
}
",
    )
    .expect("external implementor source must be written");

    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("external cargo check must execute");
    let _ = fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "external implementor failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
