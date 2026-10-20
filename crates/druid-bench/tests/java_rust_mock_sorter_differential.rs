//! Mock ExceptionSorter original Java case and physical-close cleanup differential.
#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::StatementOptions;
use sha2::{Digest, Sha256};
use standard_test_support::{TrackingFactory, TrackingState};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = include_str!("fixtures/mock_sorter_cases.tsv");
const SOURCE: &str = "core/src/test/java/com/alibaba/druid/bvt/pool/ExceptionSorterTest.java";
const SOURCE_SHA: &str = "cf1665f28399e9bb17d9e791651d4f5a3941bfe54c29cc5d93e1a7385cabedfd";
const CORPUS_SHA: &str = "69faf3454701669a2dc6a2095285a73684c8298364ed1615b05aedf09eefb952";

const HEADER: &str = "case_id\tsorter\terror_class\terror_code\tsql_state\tmessage\tdiscard_count\tactive_count\tidle_count";
async fn rust_observations() -> BTreeMap<String, String> {
    assert_eq!(
        format!("{:x}", Sha256::digest(INPUT.as_bytes())),
        CORPUS_SHA
    );
    assert_eq!(INPUT.lines().count(), 2);
    let input: Vec<_> = INPUT.lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(input, ["test_exceptionSorter", "mock", "mysql", "select 1"]);
    let state = TrackingState::configured();
    let mut builder = druid::pool::DruidPool::builder()
        .name("mock-sorter-live")
        .driver_name(input[1])
        .db_type_name(input[2])
        .factory(Arc::new(TrackingFactory::new(Arc::clone(&state))))
        .max_open(8)
        .max_idle(8)
        .pool_prepared_statements(true)
        .test_on_borrow(true);
    builder.set_filters(Some("stat")).unwrap();
    let pool = builder.build().await.expect("pool init");
    let sorter = pool
        .stat_value_and_reset()
        .exception_sorter_class_name
        .unwrap();
    let sorter = sorter.rsplit(['.', ':']).next().unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_statement(input[3], StatementOptions::default())
        .await
        .unwrap();
    statement.execute_prepared().await.unwrap();
    state.close_mock_physical();
    let failure = statement
        .close()
        .await
        .expect_err("closed physical connection");
    let error = failure.sql_exception().expect("typed SQL cleanup error");
    let fields = format!(
        "{sorter}\t{}\t{}\t{}\t{}",
        error.class_name(),
        error.error_code(),
        error.sql_state().unwrap_or("-"),
        error.message().unwrap_or("-")
    );
    connection.close().await.unwrap();
    let snapshot = pool.state();
    let row = format!(
        "{fields}\t{}\t{}\t{}",
        snapshot.discard_count, snapshot.active_count, snapshot.idle_count
    );
    pool.close().await;
    BTreeMap::from([(input[0].to_owned(), row)])
}
#[tokio::test]
async fn mock_sorter_physical_close_observation_is_complete() {
    let observed = rust_observations().await;
    assert_eq!(observed.len(), 1);
    assert!(observed["test_exceptionSorter"].starts_with(
        "MockExceptionSorter\tcom.alibaba.druid.mock.MockConnectionClosedException\t"
    ));
    assert!(observed["test_exceptionSorter"].ends_with("\t1\t0\t0"));
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
async fn pinned_java_and_rust_compare_original_mock_sorter_case() {
    let root = required("DRUID_JAVA_ORACLE_ROOT");
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath = std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath");
    let output = required("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "mock-sorter-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("Mock sorter differential artifacts: {}", output.display());
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/MockSorterCaseOracle.java");
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
        "crates/druid/tests/standard_test_support/mod.rs",
        "crates/druid/src/pool/druid_pool.rs",
        "crates/druid-bench/tests/java_rust_mock_sorter_differential.rs",
        "crates/druid-bench/tests/fixtures/MockSorterCaseOracle.java",
        "crates/druid-bench/tests/fixtures/mock_sorter_cases.tsv",
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
            .arg("MockSorterCaseOracle")
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
    let fields: Vec<_> = HEADER.split('\t').skip(1).collect();
    let mut comparison = format!(
        "case_id\tstatus\t{}\t{}\n",
        fields
            .iter()
            .map(|field| format!("java_{field}"))
            .collect::<Vec<_>>()
            .join("\t"),
        fields
            .iter()
            .map(|field| format!("rust_{field}"))
            .collect::<Vec<_>>()
            .join("\t")
    );
    let mut rust_tsv = format!("{HEADER}\n");
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
    assert_eq!(observed, rust, "unapproved Mock sorter cleanup difference");
    println!("1 original Mock sorter case: 1 exact match, 0 allowed differences");
}
