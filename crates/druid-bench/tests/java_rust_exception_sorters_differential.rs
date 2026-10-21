//! 从冻结源码重编译 Java 分类器，与 Rust 对共享异常语料逐例比较。
#[path = "approved_difference_support/allowance.rs"]
mod approved_difference_support;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use druid::core::{
    Db2ExceptionSorter, ExceptionSorter, ExceptionSorterProperties, InformixExceptionSorter,
    MySqlExceptionSorter, NullExceptionSorter, OceanBaseOracleExceptionSorter,
    OracleExceptionSorter, PgExceptionSorter, PhoenixExceptionSorter, SqlException,
    SqlExceptionCause, SybaseExceptionSorter,
};
use sha2::{Digest, Sha256};

const CASES: &str = include_str!("fixtures/exception_sorters.tsv");
const SOURCES: &str = include_str!("fixtures/exception_sorter_java_sources.tsv");
const EXPECTED_CASES: usize = 576;

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

fn sorter(name: &str) -> Box<dyn ExceptionSorter> {
    match name {
        "mysql" => Box::new(MySqlExceptionSorter),
        "pg" => Box::new(PgExceptionSorter),
        "phoenix" => Box::new(PhoenixExceptionSorter),
        "db2" => Box::new(Db2ExceptionSorter),
        "informix" => Box::new(InformixExceptionSorter),
        "sybase" => Box::new(SybaseExceptionSorter),
        "null" => Box::new(NullExceptionSorter),
        "oracle" => {
            let mut sorter = OracleExceptionSorter::new();
            sorter.set_fatal_error_codes(BTreeSet::new());
            Box::new(sorter)
        }
        "oceanbase_oracle" => {
            let mut sorter = OceanBaseOracleExceptionSorter::new();
            sorter.set_fatal_error_codes(BTreeSet::new());
            Box::new(sorter)
        }
        other => panic!("unknown sorter: {other}"),
    }
}

fn rust_results() -> BTreeMap<String, String> {
    let mut results = BTreeMap::new();
    for line in CASES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 8, "invalid case: {line}");
        let mut sorter = sorter(fields[1]);
        if fields[7] != "-" {
            sorter.config_from_properties(Some(&ExceptionSorterProperties::from([(
                "druid.oracle.fatalErrorCodes".to_owned(),
                fields[7].to_owned(),
            )])));
        }
        let mut exception = SqlException::new(
            fields[2].parse().expect("integer vendor code"),
            (fields[3] != "-").then(|| fields[3].to_owned()),
            (fields[4] != "-").then(|| fields[4].to_owned()),
        );
        match fields[5] {
            "true" => exception = exception.recoverable(),
            "false" => {}
            other => panic!("invalid recoverable flag: {other}"),
        }
        if fields[6] != "-" {
            for kind in fields[6].split(',') {
                exception = exception.with_cause(match kind {
                    "socket" => SqlExceptionCause::SocketTimeout,
                    "ordinary" => SqlExceptionCause::ClassName("java.lang.Exception".to_owned()),
                    other => panic!("unknown cause: {other}"),
                });
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sorter.is_exception_fatal(&exception)
        }))
        .map_or_else(|_| "panic".to_owned(), |fatal| fatal.to_string());
        assert!(
            results.insert(fields[0].to_owned(), result).is_none(),
            "duplicate case: {}",
            fields[0]
        );
    }
    results
}

#[test]
fn shared_corpus_has_complete_unique_rust_results_without_panics() {
    let results = rust_results();
    assert_eq!(results.len(), EXPECTED_CASES, "fixed case denominator");
    assert!(results.values().all(|result| result != "panic"));
    let vendors: BTreeSet<_> = CASES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').nth(1).unwrap())
        .collect();
    assert_eq!(
        vendors,
        BTreeSet::from([
            "mysql",
            "pg",
            "db2",
            "informix",
            "sybase",
            "oracle",
            "oceanbase_oracle",
            "null",
            "phoenix",
        ])
    );
}

#[test]
#[ignore = "requires pinned Java Druid source, compiled dependency classpath and JDK"]
fn pinned_java_and_rust_match_every_shared_exception_case() {
    let root = required_path("DRUID_JAVA_ORACLE_ROOT");
    let java_home = required_path("DRUID_JAVA_ORACLE_JAVA_HOME");
    let classpath =
        std::env::var_os("DRUID_JAVA_ORACLE_CLASSPATH").expect("frozen classpath required");
    let work = required_path("DRUID_JAVA_ORACLE_WORK_DIR");
    let output = work.join(format!(
        "exception-sorters-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).expect("create oracle output directory");
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let version = Command::new(tool(&java_home, "java"))
        .arg("-version")
        .output()
        .expect("read Java runtime version");
    assert!(
        version.status.success(),
        "Java runtime version command failed"
    );
    std::fs::write(output.join("java-version.stdout"), &version.stdout).unwrap();
    std::fs::write(output.join("java-version.stderr"), &version.stderr).unwrap();
    let metadata = format!(
        "java_commit\t33824c3dec1612711f9bb4e409319bcab2e4cd0e\ncorpus_sha256\t{:x}\nmanifest_sha256\t{:x}\njava_oracle_sha256\t{:x}\n",
        Sha256::digest(CASES.as_bytes()), Sha256::digest(SOURCES.as_bytes()),
        Sha256::digest(std::fs::read(fixture_dir.join("ExceptionSortersOracle.java")).unwrap())
    );
    std::fs::write(output.join("run-metadata.tsv"), metadata).unwrap();
    let mut source_paths = Vec::new();
    for line in SOURCES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (relative, expected) = line.split_once('\t').expect("source path and SHA-256");
        let path = root.join(relative);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(&path).expect("read pinned Java source"))
            ),
            expected,
            "source drift: {relative}"
        );
        source_paths.push(path);
    }
    assert_eq!(source_paths.len(), 11, "frozen source denominator");
    // 从校验过的源码编译，并让输出目录优先，防止其他版本的 Druid 类充当 oracle。
    let compile = Command::new(tool(&java_home, "javac"))
        .args(["-encoding", "UTF-8", "-cp"])
        .arg(&classpath)
        .arg("-d")
        .arg(&output)
        .args(&source_paths)
        .arg(fixture_dir.join("ExceptionSortersOracle.java"))
        .output()
        .expect("run pinned Java compiler");
    std::fs::write(output.join("javac.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "javac failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let mut paths = vec![output.clone()];
    paths.extend(std::env::split_paths(&classpath));
    let java = Command::new(tool(&java_home, "java"))
        .arg("-cp")
        .arg(std::env::join_paths(paths).expect("oracle classpath"))
        .arg("ExceptionSortersOracle")
        .arg(fixture_dir.join("exception_sorters.tsv"))
        .arg(output.join("java-results.tsv"))
        .output()
        .expect("run pinned Java oracle");
    std::fs::write(output.join("java.stdout"), &java.stdout).unwrap();
    std::fs::write(output.join("java.stderr"), &java.stderr).unwrap();
    assert!(
        java.status.success(),
        "Java oracle failed: {}",
        String::from_utf8_lossy(&java.stderr)
    );
    let mut java_results = BTreeMap::new();
    let java_output =
        std::fs::read_to_string(output.join("java-results.tsv")).expect("UTF-8 Java case results");
    for line in java_output.lines() {
        let (id, result) = line.split_once('\t').expect("Java case ID and result");
        assert!(
            result == "true" || result == "false" || result.starts_with("throws:"),
            "unexpected Java result: {result}"
        );
        assert!(
            java_results
                .insert(id.to_owned(), result.to_owned())
                .is_none(),
            "duplicate Java case: {id}"
        );
    }
    let rust_results = rust_results();
    let denominator: usize = CASES
        .lines()
        .next()
        .unwrap()
        .strip_prefix("# cases=")
        .expect("fixed corpus denominator")
        .parse()
        .unwrap();
    assert_eq!(
        denominator, EXPECTED_CASES,
        "review required for corpus changes"
    );
    assert_eq!(
        rust_results.len(),
        denominator,
        "missing or extra Rust cases"
    );
    assert_eq!(
        java_results.len(),
        denominator,
        "missing or extra Java cases"
    );
    let mut mismatches = Vec::new();
    let mut records = String::from("case\tjava\trust\tstatus\n");
    for (id, rust) in &rust_results {
        let java = java_results.get(id).expect("same case identity");
        let status = if java == rust {
            "MATCH"
        } else if approved_difference_support::approved("exception_sorters", id, java, rust) {
            "APPROVED_STANDARD_DIFFERENCE"
        } else {
            "MISMATCH"
        };
        writeln!(records, "{id}\t{java}\t{rust}\t{status}").expect("write comparison row");
        if status == "MISMATCH" {
            mismatches.push(id);
        }
    }
    std::fs::write(output.join("comparison.tsv"), records).unwrap();
    println!(
        "Java Druid 33824c3dec1612711f9bb4e409319bcab2e4cd0e; {} cases; artifacts: {}",
        denominator,
        output.display()
    );
    assert!(
        mismatches.is_empty(),
        "different classifications: {mismatches:?}"
    );
}
