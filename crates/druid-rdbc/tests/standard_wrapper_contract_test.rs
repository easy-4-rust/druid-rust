use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_FIXTURE: &str = include_str!("fixtures/wrapper_runtime.rs");
const WRONG_VARIANT_FIXTURE: &str = include_str!("fixtures/wrapper_wrong_variant.rs");
const DYN_DOWNCAST_FIXTURE: &str = include_str!("fixtures/wrapper_dyn_downcast.rs");
const MUT_ALIAS_FIXTURE: &str = include_str!("fixtures/wrapper_mut_alias.rs");
const MUT_ESCAPE_FIXTURE: &str = include_str!("fixtures/wrapper_mut_escape.rs");

fn run_external_fixture(name: &str, source: &str, cargo_command: &str) -> Output {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch for a unique fixture directory")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("druid-rdbc-{name}-{unique}"));
    fs::create_dir_all(root.join("src")).expect("external fixture directory must be created");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir
        .to_str()
        .expect("druid-rdbc manifest path must be valid UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
async-trait = "0.1"
druid-rdbc = {{ path = "{manifest_path}" }}
tokio = {{ version = "1", features = ["macros", "rt"] }}
"#,
    );
    fs::write(root.join("Cargo.toml"), manifest)
        .expect("external fixture manifest must be written");
    fs::write(root.join("src/main.rs"), source).expect("external fixture source must be written");

    // 外部 fixture 位于临时目录，rustup 可能选择与仓库不同的默认编译器；
    // 按实际子进程的编译器版本隔离增量产物，避免跨工具链复用 rmeta。
    let rustc_version = Command::new("rustc")
        .arg("--version")
        .current_dir(&root)
        .output()
        .expect("fixture rustc version must be available");
    assert!(
        rustc_version.status.success(),
        "fixture rustc version failed"
    );
    let version = String::from_utf8_lossy(&rustc_version.stdout);
    let version = version
        .split_whitespace()
        .nth(1)
        .expect("rustc version must include a release number")
        .replace('.', "_");

    let output = Command::new(env!("CARGO"))
        .args([cargo_command, "--offline", "--quiet"])
        .current_dir(&root)
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("DRUID_RDBC_EXTERNAL_TARGET_ROOT").map_or_else(
                || {
                    std::env::var_os("CARGO_TARGET_DIR")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| manifest_dir.join("target"))
                        .join(format!("external-wrapper-fixtures-{version}"))
                },
                |root| std::path::PathBuf::from(root).join("external-wrapper-fixtures"),
            ),
        )
        .output()
        .expect("external cargo command must execute");
    fs::remove_dir_all(&root).expect("external fixture directory must be removed");
    output
}

#[test]
fn mutable_wrapper_views_cannot_alias_the_same_owner() {
    let output = run_external_fixture("wrapper-mut-alias", MUT_ALIAS_FIXTURE, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "mutable views unexpectedly alias");
    assert!(
        stderr.contains("error[E0499]"),
        "expected exclusive borrow rejection: {stderr}"
    );
}

#[test]
fn mutable_wrapper_view_cannot_outlive_its_owner() {
    let output = run_external_fixture("wrapper-mut-escape", MUT_ESCAPE_FIXTURE, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "mutable view unexpectedly outlives its owner"
    );
    assert!(
        stderr.contains("error[E0515]"),
        "expected escaping borrow rejection: {stderr}"
    );
}

#[test]
fn external_runtime_covers_every_wrapper_variant_and_inheritance_group() {
    let output = run_external_fixture("wrapper-runtime", RUNTIME_FIXTURE, "run");
    assert!(
        output.status.success(),
        "external Wrapper runtime fixture failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn wrong_variant_macro_use_fails_with_canonical_trait_bound() {
    let output = run_external_fixture("wrapper-wrong-variant", WRONG_VARIANT_FIXTURE, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "wrong-variant fixture unexpectedly compiled"
    );
    assert!(
        stderr.contains("error[E0277]"),
        "expected E0277, got:\n{stderr}"
    );
    assert!(
        stderr.contains("WrongVariant") && stderr.contains("Connection"),
        "expected the WrongVariant -> Connection bound diagnostic, got:\n{stderr}",
    );
}

#[test]
fn dyn_wrapper_cannot_use_concrete_downcast_extension() {
    let output = run_external_fixture("wrapper-dyn-downcast", DYN_DOWNCAST_FIXTURE, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "dyn Wrapper downcast fixture unexpectedly compiled"
    );
    assert!(
        stderr.contains("error[E0277]"),
        "expected E0277, got:\n{stderr}"
    );
    assert!(
        stderr.contains("dyn druid_rdbc::wrapper::Wrapper") && stderr.contains("Sized"),
        "expected the dyn Wrapper Sized diagnostic, got:\n{stderr}",
    );
}
