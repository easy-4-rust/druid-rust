//! 从固定原始 Java 测试重编译 oracle，逐例比较真实 Rust 的异常和连接生命周期。
#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;
mod statement_error_support;

use sha2::{Digest, Sha256};
use statement_error_support::{approved_difference, cases, rust_results, CASES, DIGEST, HEADER};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE: &str = "core/src/test/java/com/alibaba/druid/bvt/pool/DruidPooledStatementTest.java";
const SOURCE_SHA: &str = "4ee9bd8bd897e9f7aae605ef5bc18afed0766a0fa2a5ff27b5c3d567b0dab40e";

fn verify_corpus() {
    assert_eq!(format!("{:x}", Sha256::digest(CASES.as_bytes())), DIGEST);
    let rows = cases();
    let unique: std::collections::BTreeSet<_> = rows.iter().map(|row| row[0]).collect();
    assert_eq!(unique.len(), 37);
}

#[tokio::test]
async fn all_frozen_statement_inputs_execute_against_rust_with_full_lifecycle_observations() {
    verify_corpus();
    let results = rust_results().await;
    assert_eq!(results.len(), 37);
    for (id, record) in results {
        let expected =
            if id == "test_closeOnCompletion_error" || id == "test_isCloseOnCompletion_error" {
                "RETURN\t-\t-\t-\t-\t0\t1\t0\t0\t1\t0\t0\t1\ttrue"
            } else {
                "THROW\tjava.sql.SQLException\t0\t-\t-\t0\t1\t0\t1\t1\t0\t0\t1\ttrue"
            };
        assert_eq!(record, expected, "{id}");
    }
}

#[test]
fn completion_allowance_rejects_unrelated_cases_or_any_observed_field_drift() {
    let java =
        "THROW\tjava.sql.SQLFeatureNotSupportedException\t0\t-\t-\t0\t1\t0\t0\t1\t0\t0\t1\ttrue";
    let rust = "RETURN\t-\t-\t-\t-\t0\t1\t0\t0\t1\t0\t0\t1\ttrue";
    for id in [
        "test_closeOnCompletion_error",
        "test_isCloseOnCompletion_error",
    ] {
        assert!(approved_difference(id, java, rust));
        assert!(!approved_difference(
            id,
            &java.replace("true", "false"),
            rust
        ));
        assert!(!approved_difference(
            id,
            java,
            &rust.replace("true", "false")
        ));
    }
    assert!(!approved_difference("test_cancel_error", java, rust));
}

fn required_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

fn tool(home: &Path, name: &str) -> PathBuf {
    home.join("bin").join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    })
}

fn save_command(output: &Path, label: &str, result: &std::process::Output) {
    std::fs::write(output.join(format!("{label}.stdout")), &result.stdout).unwrap();
    std::fs::write(output.join(format!("{label}.stderr")), &result.stderr).unwrap();
    std::fs::write(
        output.join(format!("{label}.exit")),
        result.status.to_string(),
    )
    .unwrap();
    assert!(
        result.status.success(),
        "{label} failed; evidence: {}",
        output.display()
    );
}

#[tokio::test]
#[ignore = "requires frozen Druid 1.2.28 build classpath, source and explicit JDK"]
async fn pinned_java_and_rust_compare_all_37_statement_inputs() {
    verify_corpus();
    let root = required_path("DRUID_JAVA_ORACLE_ROOT");
    let home = required_path("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath =
        std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen Java classpath required");
    let output = required_path("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "statement-errors-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("Statement differential artifacts: {}", output.display());
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source = root.join(SOURCE);
    let source_bytes = std::fs::read(&source).expect("frozen Java test source");
    assert_eq!(
        format!("{:x}", Sha256::digest(&source_bytes)),
        SOURCE_SHA,
        "Java source drift"
    );
    save_command(
        &output,
        "java-version",
        &Command::new(tool(&home, "java"))
            .arg("-version")
            .output()
            .unwrap(),
    );
    let git = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    save_command(&output, "rust-head", &git);
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    save_command(&output, "rust-worktree", &status);
    save_command(
        &output,
        "rust-toolchain",
        &Command::new("rustc").arg("-Vv").output().unwrap(),
    );
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut source_hashes = String::from("path\tsha256\n");
    for relative in [
        "Cargo.lock",
        "crates/druid-rdbc/src/statement/statement.rs",
        "crates/druid/src/core/druid_pooled_statement.rs",
        "crates/druid/src/core/druid_pooled_prepared_statement.rs",
        "crates/druid/src/core/druid_pooled_callable_statement.rs",
        "crates/druid/src/core/filter.rs",
        "crates/druid/src/core/statement_state_delegates.rs",
        "crates/druid/src/core/optional_statement_state_delegates.rs",
        "crates/druid/tests/standard_test_support/mod.rs",
        "crates/druid-bench/tests/statement_error_support/mod.rs",
        "crates/druid-bench/tests/java_rust_statement_errors_differential.rs",
    ] {
        writeln!(
            source_hashes,
            "{relative}\t{:x}",
            Sha256::digest(std::fs::read(repository.join(relative)).unwrap())
        )
        .unwrap();
    }
    std::fs::write(output.join("rust-source-sha256.tsv"), source_hashes).unwrap();
    let metadata = format!("java_commit\t33824c3dec1612711f9bb4e409319bcab2e4cd0e\nsource_sha256\t{SOURCE_SHA}\ncorpus_sha256\t{DIGEST}\noracle_sha256\t{:x}\njava_classpath\t{}\n",
        Sha256::digest(std::fs::read(fixtures.join("StatementErrorCasesOracle.java")).unwrap()), classpath.to_string_lossy());
    std::fs::write(output.join("run-metadata.tsv"), metadata).unwrap();
    std::fs::write(output.join("inputs.tsv"), CASES).unwrap();
    let compile = Command::new(tool(&home, "javac"))
        .args(["-encoding", "UTF-8", "-cp"])
        .arg(&classpath)
        .arg("-d")
        .arg(&output)
        .arg(&source)
        .arg(fixtures.join("StatementErrorCasesOracle.java"))
        .output()
        .unwrap();
    save_command(&output, "javac", &compile);
    let mut paths = vec![output.clone()];
    paths.extend(std::env::split_paths(&classpath));
    let java = Command::new(tool(&home, "java"))
        .arg(format!("-Djava.io.tmpdir={}", output.display()))
        .arg("-cp")
        .arg(std::env::join_paths(paths).unwrap())
        .arg("StatementErrorCasesOracle")
        .arg(output.join("inputs.tsv"))
        .arg(output.join("java-results.tsv"))
        .output()
        .unwrap();
    save_command(&output, "java", &java);
    let text = std::fs::read_to_string(output.join("java-results.tsv")).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some(HEADER));
    let mut java_results = BTreeMap::new();
    for line in lines {
        assert_eq!(line.split('\t').count(), 15, "complete observation row");
        let (id, observation) = line.split_once('\t').unwrap();
        assert!(
            java_results
                .insert(id.to_owned(), observation.to_owned())
                .is_none(),
            "duplicate Java case"
        );
    }
    let rust = rust_results().await;
    assert_eq!(java_results.len(), 37, "fixed Java denominator");
    assert_eq!(rust.len(), 37, "fixed Rust denominator");
    assert_eq!(
        java_results.keys().collect::<Vec<_>>(),
        rust.keys().collect::<Vec<_>>(),
        "same IDs"
    );
    let mut rust_tsv = format!("{HEADER}\n");
    let mut comparison = String::from("case_id\tstatus");
    for prefix in ["java", "rust"] {
        for field in HEADER.split('\t').skip(1) {
            write!(comparison, "\t{prefix}_{field}").unwrap();
        }
    }
    comparison.push('\n');
    let (mut matched, mut allowed, mut mismatches) = (0, 0, Vec::new());
    for (id, observation) in rust {
        writeln!(rust_tsv, "{id}\t{observation}").unwrap();
        let java = &java_results[&id];
        let status = if java == &observation {
            matched += 1;
            "MATCH"
        } else if approved_difference(&id, java, &observation) {
            allowed += 1;
            "APPROVED_STANDARD_DIFFERENCE"
        } else {
            mismatches.push(id.clone());
            "MISMATCH"
        };
        writeln!(comparison, "{id}\t{status}\t{java}\t{observation}").unwrap();
    }
    std::fs::write(output.join("rust-results.tsv"), rust_tsv).unwrap();
    assert!(
        comparison
            .lines()
            .all(|line| line.split('\t').count() == 30),
        "fully named Java/Rust observation columns"
    );
    std::fs::write(output.join("comparison.tsv"), comparison).unwrap();
    println!(
        "37 cases: {matched} exact matches, {allowed} approved standard differences, {} mismatches",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "unapproved differences: {mismatches:?}; {}",
        output.display()
    );
    assert_eq!(
        (matched, allowed),
        (35, 2),
        "approved baseline must not silently change"
    );
}
