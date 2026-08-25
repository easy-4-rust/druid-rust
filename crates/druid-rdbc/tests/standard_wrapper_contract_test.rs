use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_FIXTURE: &str = include_str!("fixtures/wrapper_runtime.rs");
const WRONG_VARIANT_FIXTURE: &str = include_str!("fixtures/wrapper_wrong_variant.rs");
const DYN_DOWNCAST_FIXTURE: &str = include_str!("fixtures/wrapper_dyn_downcast.rs");

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

    let output = Command::new(env!("CARGO"))
        .args([cargo_command, "--offline", "--quiet"])
        .current_dir(&root)
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("target/external-wrapper-fixtures"),
        )
        .output()
        .expect("external cargo command must execute");
    fs::remove_dir_all(&root).expect("external fixture directory must be removed");
    output
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
