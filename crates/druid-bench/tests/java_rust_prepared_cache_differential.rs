//! 冻结 PreparedStatement 缓存用例：属性恢复、结果遍历及语句诊断可观察性。
#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{
    FetchDirection, ParameterIndex, PreparedParameter, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
#[path = "prepared_cache_case_support/mock_driver.rs"]
mod mock_driver;
use sha2::{Digest, Sha256};
use standard_test_support::TrackingState;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = include_str!("fixtures/prepared_cache_cases.tsv");
const SOURCE: &str =
    "core/src/test/java/com/alibaba/druid/bvt/pool/DruidPooledPreparedStatementTest.java";
const SOURCE_SHA: &str = "a5cc279eab7207b19c3cb98ff0a9c85cafa1e5db9ef7f5dc2ab91d976b62ce9a";
const CORPUS_SHA: &str = "7681fa4dac8ffdb02343fb72a5868f39a5f5df3b7c66f5590f7a9d27c26f2d10";

const HEADER: &str = "case_id\tbefore_first\tmutated\tbefore_second\tfirst_rows\tsecond_rows\tsql\tfirst_hit\tsecond_hit\tkey_present\tpool_hits\tactive_count\tidle_count";
fn properties(statement: &dyn Statement) -> String {
    format!(
        "{},{},{},{},{}",
        statement.fetch_size().unwrap(),
        statement.fetch_direction().unwrap().rdbc_code(),
        statement.max_rows().unwrap(),
        statement.max_field_size().unwrap(),
        statement.query_timeout().unwrap()
    )
}
async fn rust_observations() -> BTreeMap<String, String> {
    assert_eq!(
        format!("{:x}", Sha256::digest(INPUT.as_bytes())),
        CORPUS_SHA
    );
    assert_eq!(INPUT.lines().count(), 2);
    let input: Vec<_> = INPUT.lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(input, ["test_hitCount", "select ?", "xx", "1,2,3,4,5"]);
    let state = TrackingState::configured();
    let mut builder = druid::pool::DruidPool::builder()
        .name("prepared-cache-live")
        .driver_name("mock")
        .factory(Arc::new(mock_driver::Factory(
            Arc::clone(&state),
            Arc::new(AtomicUsize::new(0)),
        )))
        .max_open(8)
        .max_idle(8)
        .pool_prepared_statements(true)
        .test_on_borrow(false);
    builder.set_filters(Some("stat")).unwrap();
    let pool = builder.build().await.unwrap();
    let mut initial = Vec::new();
    let mut traversal = Vec::new();
    let mut mutated = String::new();
    let mut sql = String::new();
    for round in 0..2 {
        let mut connection = pool.get().await.unwrap();
        let mut statement = connection
            .prepare_statement(input[1], StatementOptions::default())
            .await
            .unwrap();
        initial.push(properties(statement.as_ref()));
        if round == 0 {
            sql = statement.sql().unwrap().to_owned();
            statement.set_fetch_size(1).unwrap();
            // Java 原测试使用非 JDBC 整数 2；DriverValue 保留该输入，不改成标准常量。
            statement
                .set_fetch_direction(FetchDirection::DriverValue(2))
                .unwrap();
            statement.set_max_rows(3).unwrap();
            statement.set_max_field_size(4).unwrap();
            statement.set_query_timeout(5).unwrap();
            mutated = properties(statement.as_ref());
        }
        statement
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String(input[2].to_owned()),
            })
            .unwrap();
        let mut result = statement.execute_prepared_query().await.unwrap();
        traversal.push(format!(
            "{},{}",
            result.next().await.unwrap(),
            result.next().await.unwrap()
        ));
        result.close().await.unwrap();
        connection.close().await.unwrap();
    }
    let snapshot = pool.state();
    // 标准 trait 未暴露 Java Druid 专有 getHitCount/getKey，不能用池级数值替代。
    let row = format!(
        "{}\t{mutated}\t{}\t{}\t{}\t{sql}\tNOT_EXPOSED\tNOT_EXPOSED\tNOT_EXPOSED\t{}\t{}\t{}",
        initial[0],
        initial[1],
        traversal[0],
        traversal[1],
        snapshot.cached_prepared_statement_hit_count,
        snapshot.active_count,
        snapshot.idle_count
    );
    pool.close().await;
    BTreeMap::from([(input[0].to_owned(), row)])
}
#[tokio::test]
async fn prepared_cache_observation_keeps_the_complete_original_case() {
    let observed = rust_observations().await;
    assert_eq!(observed.len(), 1);
    assert!(observed["test_hitCount"].contains("1,2,3,4,5"));
    assert!(observed["test_hitCount"].contains("true,false\ttrue,false"));
    assert!(observed["test_hitCount"].contains("NOT_EXPOSED\tNOT_EXPOSED\tNOT_EXPOSED"));
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
async fn pinned_java_and_rust_compare_original_prepared_cache_case() {
    let root = required("DRUID_JAVA_ORACLE_ROOT");
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath = std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath");
    let output = required("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "prepared-cache-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!(
        "Prepared cache differential artifacts: {}",
        output.display()
    );
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/PreparedCacheCaseOracle.java");
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
        "crates/druid-bench/tests/prepared_cache_case_support/mock_driver.rs",
        "crates/druid/src/core/druid_pooled_prepared_statement.rs",
        "crates/druid/tests/standard_test_support/mod.rs",
        "crates/druid/src/pool/druid_pool.rs",
        "crates/druid-bench/tests/java_rust_prepared_cache_differential.rs",
        "crates/druid-bench/tests/fixtures/PreparedCacheCaseOracle.java",
        "crates/druid-bench/tests/fixtures/prepared_cache_cases.tsv",
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
            .arg("PreparedCacheCaseOracle")
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
    assert_eq!(
        observed, rust,
        "unapproved prepared cache observation difference"
    );
    println!("1 original prepared cache case: 1 exact match, 0 allowed differences");
}
