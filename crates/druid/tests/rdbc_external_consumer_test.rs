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
    let manifest = format!(
        "[package]\nname = \"rdbc-external-consumer-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\ndruid = {{ path = \"{}\" }}\n",
        toml_path(&druid_path)?,
    );
    let source = r#"use druid::sql::{Connection, DataSource, Date, ResultSet, SQLException, Statement};

fn accepts_connection(_: Option<Connection>) {}
fn accepts_result_set(_: Option<ResultSet>) {}

fn main() {
    accepts_connection(None);
    accepts_result_set(None);
    let _ = std::any::TypeId::of::<Date>();
}
"#;
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
