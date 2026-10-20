//! 冻结 WallFilterTest 的 H2 实库、原始 SQL、资源边界与表统计检查点。
#![cfg(feature = "jdbc-agent")]
use druid::core::FilterChain;
use druid::pool::{DruidPool, PoolInnerConfig};
use druid::sql::{WallConfig, WallFilter, WallProvider};
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const INPUT: &str = include_str!("fixtures/wall_table_cases.tsv");
const SOURCE: &str = "core/src/test/java/com/alibaba/druid/bvt/filter/wall/WallFilterTest.java";
const SOURCE_SHA: &str = "269698f1122383bb22fa8b58df06770d0d29e938319b9a88aa885d6a1ebb2569";
const CORPUS_SHA: &str = "0494e11cb12bc5bc9e139fbd861fc5fcdf7ce8fd09008fc192f3dbbb03367dbf";

const HEADER: &str = "case_id\tcheckpoints";
fn verify_inputs() {
    assert_eq!(
        format!("{:x}", Sha256::digest(INPUT.as_bytes())),
        CORPUS_SHA
    );
    assert_eq!(INPUT.lines().count(), 21);
    assert_eq!(
        INPUT.lines().next(),
        Some("step\toperation\tsql\tparameters\tcheckpoint")
    );
    for (step, line) in INPUT.lines().skip(1).enumerate() {
        let input: Vec<_> = line.split('\t').collect();
        assert_eq!(input.len(), 5);
        assert_eq!(input[0], step.to_string());
    }
}
async fn rust_observations() -> BTreeMap<String, String> {
    verify_inputs();
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let options = JdbcAgentOptions::java(
        tool(&home, "java"),
        [
            required("DRUID_JDBC_AGENT_JAR"),
            required("DRUID_JDBC_DRIVER_JAR"),
        ],
    )
    .unwrap();
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::new(Arc::clone(&provider))));
    let pool = DruidPool::new_standard(
        "wall-table-live".to_owned(),
        "h2".to_owned(),
        Arc::new(JdbcAgentConnectionFactory::new(
            "jdbc:h2:mem:wall_test;",
            Some("SELECT 1".to_owned()),
            options.runtime_identity("wall-table-live", "local", "isolated"),
        )),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.unwrap();
    let mut checkpoints = Vec::new();
    // 原 Java 将十条 INSERT 放在同一借用中，其余操作各自关闭语句和连接。
    let mut connection = None;
    let mut inserts = None;
    for line in INPUT.lines().skip(1) {
        let input: Vec<_> = line.split('\t').collect();
        let step: usize = input[0].parse().unwrap();
        if step == 1 {
            let mut c = pool.get().await.unwrap();
            inserts = Some(
                c.create_statement(StatementOptions::default())
                    .await
                    .unwrap(),
            );
            connection = Some(c);
        }
        if (1..=10).contains(&step) {
            inserts
                .as_mut()
                .unwrap()
                .execute(input[2], GeneratedKeys::None)
                .await
                .unwrap();
            if step == 10 {
                inserts.as_mut().unwrap().close().await.unwrap();
                connection.as_mut().unwrap().close().await.unwrap();
                inserts.take();
                connection.take();
            }
            continue;
        }
        let mut c = pool.get().await.unwrap();
        match input[1] {
            "statement" => {
                let mut s = c
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                s.execute(input[2], GeneratedKeys::None).await.unwrap();
                s.close().await.unwrap();
            }
            "query" => {
                let mut s = c
                    .prepare_statement(input[2], StatementOptions::default())
                    .await
                    .unwrap();
                let mut r = s.execute_prepared_query().await.unwrap();
                while r.next().await.unwrap() {}
                r.close().await.unwrap();
                s.close().await.unwrap();
            }
            "prepared" | "batch" => {
                let mut s = c
                    .prepare_statement(input[2], StatementOptions::default())
                    .await
                    .unwrap();
                for (index, id) in input[3].split(',').enumerate() {
                    s.set_parameter(PreparedParameter {
                        index: ParameterIndex::new(if input[1] == "batch" { 1 } else { index + 1 })
                            .unwrap(),
                        value: Value::Int(id.parse().unwrap()),
                    })
                    .unwrap();
                    if input[1] == "batch" {
                        s.add_prepared_batch().unwrap();
                    }
                }
                if input[1] == "batch" {
                    s.execute_prepared_batch().await.unwrap();
                } else {
                    s.execute_prepared().await.unwrap();
                }
                s.close().await.unwrap();
            }
            _ => panic!("unknown operation"),
        }
        c.close().await.unwrap();
        let stat = provider
            .table_stat_values(false)
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case("t"))
            .unwrap();
        let count = match input[4] {
            "create_count" => stat.create_count,
            "fetch_row_count" => stat.fetch_row_count,
            "delete_data_count" => stat.delete_data_count,
            "update_data_count" => stat.update_data_count,
            "truncate_count" => stat.truncate_count,
            "drop_count" => stat.drop_count,
            _ => panic!("unknown checkpoint"),
        };
        checkpoints.push(count.to_string());
    }
    pool.close().await;
    BTreeMap::from([("test_wallFilter".to_owned(), checkpoints.join(","))])
}
#[test]
fn wall_table_corpus_retains_all_twenty_original_operations() {
    verify_inputs();
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
async fn pinned_java_and_rust_compare_original_wall_table_case() {
    let root = required("DRUID_JAVA_ORACLE_ROOT");
    let home = required("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath = std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath");
    let output = required("DRUID_JAVA_ORACLE_WORK_DIR").join(format!(
        "wall-table-live-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    println!("Wall table differential artifacts: {}", output.display());
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/WallTableCaseOracle.java");
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
        "crates/druid/src/sql/wall_filter.rs",
        "crates/druid/src/sql/wall_provider.rs",
        "crates/druid/src/pool/druid_pool.rs",
        "crates/druid-bench/tests/java_rust_wall_table_differential.rs",
        "crates/druid-bench/tests/fixtures/WallTableCaseOracle.java",
        "crates/druid-bench/tests/fixtures/wall_table_cases.tsv",
    ] {
        writeln!(
            hashes,
            "{path}\t{:x}",
            Sha256::digest(std::fs::read(repository.join(path)).unwrap())
        )
        .unwrap();
    }
    for (label, path) in [
        ("jdbc-agent.jar", required("DRUID_JDBC_AGENT_JAR")),
        ("h2-driver.jar", required("DRUID_JDBC_DRIVER_JAR")),
    ] {
        writeln!(
            hashes,
            "{label}\t{:x}",
            Sha256::digest(std::fs::read(path).unwrap())
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
            .arg("WallTableCaseOracle")
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
        "unapproved Wall table statistics difference"
    );
    println!("1 original Wall table case: 1 exact match, 0 allowed differences");
}
