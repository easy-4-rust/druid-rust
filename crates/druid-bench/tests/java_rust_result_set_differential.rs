//! 冻结五例 ResultSet 的实时双端对照；未经确认的差异保持失败。
mod result_set_case_support;
#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;

use result_set_case_support::{cases, rust_results, CASES, DIGEST, HEADER};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
const SOURCES: &str = include_str!("fixtures/result_set_java_sources.tsv");

fn verify_corpus() {
    assert_eq!(format!("{:x}", Sha256::digest(CASES.as_bytes())), DIGEST);
    let rows = cases();
    let ids: BTreeSet<_> = rows.iter().map(|row| row[0]).collect();
    assert_eq!(ids.len(), 5);
}

#[tokio::test]
async fn all_original_result_set_inputs_have_complete_rust_observations_and_cleanup() {
    verify_corpus();
    assert!(result_set_case_support::raw_fixture_standard_unwrap_preserves_identity());
    let records = rust_results().await;
    assert_eq!(records.len(), 5);
    for (id, record) in records {
        let fields: Vec<_> = record.split('\t').collect();
        assert_eq!(fields.len(), 14);
        assert_eq!(
            &fields[7..],
            ["1", "0", "true", "true", "0", "1", "true"],
            "{id}"
        );
        if id.ends_with("testWrap") {
            assert_eq!(
                fields[0], "raw=false,logical=true",
                "canonical Rust role remains governed"
            );
            assert_eq!(fields[1], "RETURN");
        } else if id.ends_with("test_rowCount") {
            assert_eq!(fields[0], "true,false,1,true,false,1,true,false,1,false");
            assert_eq!(fields[1], "RETURN");
        } else {
            assert_eq!(fields[1], "THROW");
            assert_eq!(fields[2], "java.sql.SQLFeatureNotSupportedException");
            assert_eq!(&fields[3..7], ["0", "-", "-", "1"]);
        }
    }
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
#[ignore = "requires frozen Java 1.2.28 source/build classpath and explicit JDK; unapproved differences fail"]
async fn pinned_java_and_rust_compare_all_five_result_set_inputs() {
    verify_corpus();
    let root = required_path("DRUID_JAVA_ORACLE_ROOT");
    let home = required_path("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath =
        std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath required");
    let output = required_path("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "result-set-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("ResultSet differential artifacts: {}", output.display());
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut test_sources = Vec::new();
    let mut source_count = 0;
    for line in SOURCES.lines().filter(|line| !line.starts_with('#')) {
        let (relative, expected) = line.split_once('\t').unwrap();
        let source = root.join(relative);
        assert_eq!(
            format!("{:x}", Sha256::digest(std::fs::read(&source).unwrap())),
            expected,
            "frozen source drift: {relative}"
        );
        if relative.contains("/src/test/") {
            test_sources.push(source);
        }
        source_count += 1;
    }
    assert_eq!(source_count, 7);
    assert_eq!(test_sources.len(), 2);
    save_command(
        &output,
        "java-version",
        &Command::new(tool(&home, "java"))
            .arg("-version")
            .output()
            .unwrap(),
    );
    save_command(
        &output,
        "rust-version",
        &Command::new("rustc").arg("-Vv").output().unwrap(),
    );
    save_command(
        &output,
        "rust-head",
        &Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap(),
    );
    save_command(
        &output,
        "rust-worktree",
        &Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .unwrap(),
    );
    std::fs::write(output.join("inputs.tsv"), CASES).unwrap();
    let metadata = format!("java_commit\t33824c3dec1612711f9bb4e409319bcab2e4cd0e\ncorpus_sha256\t{DIGEST}\nsource_manifest_sha256\t{:x}\noracle_sha256\t{:x}\njava_classpath\t{}\n",Sha256::digest(SOURCES.as_bytes()),Sha256::digest(std::fs::read(fixtures.join("ResultSetCasesOracle.java")).unwrap()),classpath.to_string_lossy());
    std::fs::write(output.join("run-metadata.tsv"), metadata).unwrap();
    let compile = Command::new(tool(&home, "javac"))
        .args(["-encoding", "UTF-8", "-cp"])
        .arg(&classpath)
        .arg("-d")
        .arg(&output)
        .args(test_sources)
        .arg(fixtures.join("ResultSetCasesOracle.java"))
        .output()
        .unwrap();
    save_command(&output, "javac", &compile);
    let mut paths = vec![output.clone()];
    paths.extend(std::env::split_paths(&classpath));
    let java = Command::new(tool(&home, "java"))
        .arg(format!("-Djava.io.tmpdir={}", output.display()))
        .arg("-cp")
        .arg(std::env::join_paths(paths).unwrap())
        .arg("ResultSetCasesOracle")
        .arg(output.join("inputs.tsv"))
        .arg(output.join("java-results.tsv"))
        .output()
        .unwrap();
    save_command(&output, "java", &java);
    let java_text = std::fs::read_to_string(output.join("java-results.tsv")).unwrap();
    let mut lines = java_text.lines();
    assert_eq!(lines.next(), Some(HEADER));
    let mut java_records = BTreeMap::new();
    for line in lines {
        assert_eq!(line.split('\t').count(), 15);
        let (id, value) = line.split_once('\t').unwrap();
        assert!(java_records
            .insert(id.to_owned(), value.to_owned())
            .is_none());
    }
    let rust = rust_results().await;
    let mut rust_text = format!("{HEADER}\n");
    for (id, value) in &rust {
        writeln!(rust_text, "{id}\t{value}").unwrap();
    }
    std::fs::write(output.join("rust-results.tsv"), rust_text).unwrap();
    assert_eq!(java_records.len(), 5);
    assert_eq!(rust.len(), 5);
    assert_eq!(
        java_records.keys().collect::<Vec<_>>(),
        rust.keys().collect::<Vec<_>>()
    );
    let mut comparison = String::from("case_id\tstatus");
    for prefix in ["java", "rust"] {
        for field in HEADER.split('\t').skip(1) {
            write!(comparison, "\t{prefix}_{field}").unwrap();
        }
    }
    comparison.push('\n');
    let mut mismatches = Vec::new();
    for (id, value) in &rust {
        let java = &java_records[id];
        let status = if java == value {
            "MATCH"
        } else {
            mismatches.push(id.clone());
            "MISMATCH"
        };
        writeln!(comparison, "{id}\t{status}\t{java}\t{value}").unwrap();
    }
    assert!(comparison
        .lines()
        .all(|line| line.split('\t').count() == 30));
    std::fs::write(output.join("comparison.tsv"), comparison).unwrap();
    println!(
        "5 ResultSet cases: {} matches, {} unapproved differences",
        5 - mismatches.len(),
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "unapproved ResultSet differences: {mismatches:?}; {}",
        output.display()
    );
}
