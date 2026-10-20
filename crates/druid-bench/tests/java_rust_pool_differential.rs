//! 冻结三类六例 Pool 输入的实时 Java/Rust 对照；差异保留为失败。
#[path = "pool_case_support/cases.rs"]
mod pool_case_support;
#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;
use pool_case_support::{rust_observations, HEADER, INPUT};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
const SOURCES: &str = include_str!("fixtures/pool_java_sources.tsv");
const CORPUS_SHA: &str = "09dd147f9f41fa33a9950f67b23d3d21d54819ed873d61f81c11f8058a1b596d";
#[tokio::test]
async fn original_pool_inputs_have_complete_rust_lifecycle_records() {
    assert_eq!(
        format!("{:x}", Sha256::digest(INPUT.as_bytes())),
        CORPUS_SHA
    );
    let records = rust_observations().await;
    for (id, row) in records {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 14);
        if id.ends_with("_error") && id.contains("getConnection") {
            assert_eq!(fields[0], "THROW");
            assert_eq!(fields[6], "0");
            assert_eq!(&fields[12..], ["0", "0"]);
        } else {
            assert_eq!(fields[0], "RETURN");
            assert_eq!(fields[10], "true");
            assert_eq!(&fields[12..], ["0", "1"]);
        }
    }
}
fn required(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

fn tool(home: &Path, name: &str) -> PathBuf {
    home.join("bin").join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    })
}

fn run(output: &Path, label: &str, command: &mut Command) {
    let result = command.output().expect("start oracle tool");
    std::fs::write(output.join(format!("{label}.stdout")), result.stdout).unwrap();
    std::fs::write(output.join(format!("{label}.stderr")), result.stderr).unwrap();
    std::fs::write(
        output.join(format!("{label}.exit")),
        result.status.to_string(),
    )
    .unwrap();
    assert!(
        result.status.success(),
        "{label} failed: {}",
        output.display()
    );
}

#[tokio::test]
#[ignore = "requires frozen Druid 1.2.28 source/build classpath and explicit JDK"]
async fn pinned_java_and_rust_compare_all_six_pool_cases() {
    let root = required("DRUID_JAVA_ORACLE_ROOT");
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath = std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath");
    let output = required("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "pool-six-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("Pool differential artifacts: {}", output.display());
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/PoolCasesOracle.java");
    let sources: Vec<_> = SOURCES
        .lines()
        .skip(1)
        .map(|line| {
            let (path, sha) = line.split_once('\t').unwrap();
            let source = root.join(path);
            assert_eq!(
                format!("{:x}", Sha256::digest(std::fs::read(&source).unwrap())),
                sha
            );
            source
        })
        .collect();
    std::fs::write(output.join("inputs.tsv"), INPUT).unwrap();
    run(
        &output,
        "java-version",
        Command::new(tool(&home, "java")).arg("-version"),
    );
    run(&output, "rust-toolchain", Command::new("rustc").arg("-Vv"));
    run(
        &output,
        "rust-head",
        Command::new("git").args(["rev-parse", "HEAD"]),
    );
    run(
        &output,
        "rust-worktree",
        Command::new("git").args(["status", "--porcelain"]),
    );
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut hashes = String::from("path\tsha256\n");
    for path in [
        "Cargo.lock",
        "crates/druid-rdbc/src/data_source/data_source.rs",
        "crates/druid/src/pool/druid_data_source.rs",
        "crates/druid/src/core/druid_pooled_connection.rs",
        "crates/druid/tests/standard_test_support/mod.rs",
        "crates/druid-bench/tests/pool_case_support/cases.rs",
        "crates/druid-bench/tests/java_rust_pool_differential.rs",
        "crates/druid-bench/tests/fixtures/PoolCasesOracle.java",
        "crates/druid-bench/tests/fixtures/pool_cases.tsv",
    ] {
        writeln!(
            hashes,
            "{path}\t{:x}",
            Sha256::digest(std::fs::read(repository.join(path)).unwrap())
        )
        .unwrap();
    }
    std::fs::write(output.join("tested-source-sha256.tsv"), hashes).unwrap();
    std::fs::write(output.join("metadata.tsv"), format!(
        "java_commit\t33824c3dec1612711f9bb4e409319bcab2e4cd0e\ncorpus_sha256\t{CORPUS_SHA}\nclasspath\t{}\n", classpath.to_string_lossy())).unwrap();
    run(
        &output,
        "javac",
        Command::new(tool(&home, "javac"))
            .args(["-encoding", "UTF-8", "-cp"])
            .arg(&classpath)
            .arg("-d")
            .arg(&output)
            .args(sources)
            .arg(fixture),
    );
    let mut paths = vec![output.clone()];
    paths.extend(std::env::split_paths(&classpath));
    run(
        &output,
        "java",
        Command::new(tool(&home, "java"))
            .arg(format!("-Djava.io.tmpdir={}", output.display()))
            .arg("-cp")
            .arg(std::env::join_paths(paths).unwrap())
            .arg("PoolCasesOracle")
            .arg(output.join("inputs.tsv"))
            .arg(output.join("java-results.tsv")),
    );
    let java = std::fs::read_to_string(output.join("java-results.tsv")).unwrap();
    let mut lines = java.lines();
    assert_eq!(lines.next(), Some(HEADER));
    let mut observed = BTreeMap::new();
    for line in lines {
        let (id, valid) = line.split_once('\t').expect("complete Java row");
        assert!(observed.insert(id.to_owned(), valid.to_owned()).is_none());
    }
    let rust = rust_observations().await;
    assert_eq!(
        observed.keys().collect::<Vec<_>>(),
        rust.keys().collect::<Vec<_>>()
    );
    let mut comparison = String::from("case_id\tstatus");
    for prefix in ["java", "rust"] {
        for field in HEADER.split('\t').skip(1) {
            write!(comparison, "\t{prefix}_{field}").unwrap();
        }
    }
    comparison.push('\n');
    let mut rust_tsv = format!("{HEADER}\n");
    for (id, valid) in &rust {
        let status = if &observed[id] == valid {
            "MATCH"
        } else {
            "MISMATCH"
        };
        writeln!(comparison, "{id}\t{status}\t{}\t{valid}", observed[id]).unwrap();
        writeln!(rust_tsv, "{id}\t{valid}").unwrap();
    }
    std::fs::write(output.join("comparison.tsv"), comparison).unwrap();
    std::fs::write(output.join("rust-results.tsv"), rust_tsv).unwrap();
    assert_eq!(observed, rust, "unapproved Pool difference");
    println!("6 original Pool cases compared with no allowance path");
}
