//! 冻结 Java WallMultiStatementTest 两例的实时差分，保留原 SQL 与完整分母。
use druid::sql::WallUtils;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = include_str!("fixtures/wall_multi_cases.tsv");
const SOURCE: &str =
    "core/src/test/java/com/alibaba/druid/bvt/filter/wall/WallMultiStatementTest.java";
const SOURCE_SHA: &str = "ac9ef98f31cfcfa8a1d821cccec657072d2f06f2a72ea617913986d124496fb7";
const CORPUS_SHA: &str = "682ab4352eafee9f3f667c3a09d8e8133561e78acc79b38b773bec65f659b8b9";

fn rust_observations() -> BTreeMap<String, bool> {
    assert_eq!(
        format!("{:x}", Sha256::digest(INPUT.as_bytes())),
        CORPUS_SHA
    );
    let mut result = BTreeMap::new();
    assert_eq!(INPUT.lines().next(), Some("case_id\tdialect\tsql"));
    for line in INPUT.lines().skip(1) {
        let input: Vec<_> = line.split('\t').collect();
        assert_eq!(input.len(), 3);
        let valid = match (input[0], input[1]) {
            ("testOracle", "oracle") => WallUtils::is_validate_oracle(input[2]),
            ("testMySql", "mysql") => WallUtils::is_validate_my_sql(input[2]),
            _ => panic!("unknown method/dialect input"),
        }
        .expect("Wall validation returns a decision");
        assert!(result.insert(input[0].to_owned(), valid).is_none());
    }
    assert_eq!(result.len(), 2);
    result
}

#[test]
fn both_original_wall_multi_inputs_are_rejected() {
    assert!(rust_observations().values().all(|valid| !valid));
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

#[test]
#[ignore = "requires frozen Druid 1.2.28 source/build classpath and explicit JDK"]
fn pinned_java_and_rust_compare_both_original_wall_multi_cases() {
    let root = required("DRUID_JAVA_ORACLE_ROOT");
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath = std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath");
    let output = required("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "wall-multi-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("Wall differential artifacts: {}", output.display());
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/WallMultiCasesOracle.java");
    let source = root.join(SOURCE);
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&source).unwrap())),
        SOURCE_SHA
    );
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
        "crates/druid/src/sql/wall_utils.rs",
        "crates/druid-bench/tests/java_rust_wall_multi_differential.rs",
        "crates/druid-bench/tests/fixtures/WallMultiCasesOracle.java",
        "crates/druid-bench/tests/fixtures/wall_multi_cases.tsv",
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
        "java_commit\t33824c3dec1612711f9bb4e409319bcab2e4cd0e\nsource_sha256\t{SOURCE_SHA}\ncorpus_sha256\t{CORPUS_SHA}\nclasspath\t{}\n", classpath.to_string_lossy())).unwrap();
    run(
        &output,
        "javac",
        Command::new(tool(&home, "javac"))
            .args(["-encoding", "UTF-8", "-cp"])
            .arg(&classpath)
            .arg("-d")
            .arg(&output)
            .arg(source)
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
            .arg("WallMultiCasesOracle")
            .arg(output.join("inputs.tsv"))
            .arg(output.join("java-results.tsv")),
    );
    let java = std::fs::read_to_string(output.join("java-results.tsv")).unwrap();
    let mut lines = java.lines();
    assert_eq!(lines.next(), Some("case_id\tvalid"));
    let mut observed = BTreeMap::new();
    for line in lines {
        let (id, valid) = line.split_once('\t').expect("complete Java row");
        assert!(observed
            .insert(id.to_owned(), valid.parse::<bool>().unwrap())
            .is_none());
    }
    let rust = rust_observations();
    assert_eq!(
        observed.keys().collect::<Vec<_>>(),
        rust.keys().collect::<Vec<_>>()
    );
    let mut comparison = String::from("case_id\tstatus\tjava_valid\trust_valid\n");
    let mut rust_tsv = String::from("case_id\tvalid\n");
    for (id, valid) in &rust {
        let status = if observed[id] == *valid {
            "MATCH"
        } else {
            "MISMATCH"
        };
        writeln!(comparison, "{id}\t{status}\t{}\t{valid}", observed[id]).unwrap();
        writeln!(rust_tsv, "{id}\t{valid}").unwrap();
    }
    std::fs::write(output.join("comparison.tsv"), comparison).unwrap();
    std::fs::write(output.join("rust-results.tsv"), rust_tsv).unwrap();
    assert_eq!(observed, rust, "unapproved Wall decision difference");
    println!("2 original Wall cases: 2 exact matches, 0 allowed differences");
}
