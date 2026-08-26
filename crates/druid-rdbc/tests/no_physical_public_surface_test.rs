use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMPORARY_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporaryPackage {
    path: PathBuf,
}

impl TemporaryPackage {
    fn create(name: &str) -> Result<Self, Box<dyn Error>> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sequence = TEMPORARY_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "druid-rdbc-no-physical-{name}-{}-{elapsed}-{sequence}",
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
    let path = path.to_str().ok_or("package path is not valid UTF-8")?;
    Ok(path.replace('\\', "\\\\").replace('"', "\\\""))
}

fn check_external(name: &str, source: &str) -> Result<Output, Box<dyn Error>> {
    let package = TemporaryPackage::create(name)?;
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    fs::create_dir(package.path.join("src"))?;
    fs::write(
        package.path.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\ndruid-rdbc = {{ path = \"{}\" }}\n",
            toml_path(&manifest_dir)?
        ),
    )?;
    fs::write(package.path.join("src/main.rs"), source)?;
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet"])
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("target/no-physical-public-surface"),
        )
        .current_dir(&package.path)
        .output()?;
    Ok(output)
}

#[test]
fn external_public_surface_exposes_only_the_canonical_standard() -> Result<(), Box<dyn Error>> {
    let canonical = check_external(
        "canonical-standard-consumer",
        r"
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::metadata::{DatabaseMetaData, ResultSetMetaData};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement};

fn accepts_connection(_: Option<Box<dyn Connection>>) {}
fn accepts_factory(_: Option<&dyn ConnectionFactory>) {}
fn accepts_statement(_: Option<Box<dyn Statement>>) {}
fn accepts_prepared(_: Option<Box<dyn PreparedStatement>>) {}
fn accepts_callable(_: Option<Box<dyn CallableStatement>>) {}
fn accepts_result_set(_: Option<Box<dyn ResultSet>>) {}
fn accepts_database_metadata(_: Option<Box<dyn DatabaseMetaData>>) {}
fn accepts_result_set_metadata(_: Option<&dyn ResultSetMetaData>) {}

fn main() {
    accepts_connection(None);
    accepts_factory(None);
    accepts_statement(None);
    accepts_prepared(None);
    accepts_callable(None);
    accepts_result_set(None);
    accepts_database_metadata(None);
    accepts_result_set_metadata(None);
}
",
    )?;
    assert!(
        canonical.status.success(),
        "canonical external consumer failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&canonical.stdout),
        String::from_utf8_lossy(&canonical.stderr)
    );

    let legacy = check_external(
        "legacy-physical-consumer",
        r"
use druid_rdbc::spi::PhysicalConnection;

fn accepts_legacy(_: Option<Box<dyn PhysicalConnection>>) {}

fn main() {
    accepts_legacy(None);
}
",
    )?;
    assert!(
        !legacy.status.success(),
        "legacy Physical/SPI fixture unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&legacy.stderr);
    assert!(
        stderr.contains("could not find `spi`") || stderr.contains("unresolved import"),
        "legacy fixture failed for the wrong reason:\n{stderr}"
    );
    Ok(())
}
