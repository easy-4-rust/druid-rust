//! 统一驱动证据矩阵 runner。
//!
//! 对每个产品档案输出连接、查询、更新、预编译、元数据、类型、NULL、批处理、
//! 事务、保存点、取消、关闭与异常边界。未配置运行时的产品写 `NOT_RUN`，不把
//! 目录存在或 ignored 当作通过。当前本地可执行产品为 SQLite（SQLx provider）。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, RdbcErrorKind};
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use druid_wrapper::driver::{DatabaseConnectionConfig, DruidDriverRegistry};

const OPERATIONS: [&str; 13] = [
    "connect",
    "query",
    "update",
    "prepared",
    "metadata",
    "types",
    "null",
    "batch",
    "transaction",
    "savepoint",
    "cancel",
    "close",
    "error",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("DRIVER_MATRIX_FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| "无法定位仓库根目录".to_owned())?
        .to_path_buf();
    let output = arguments(&root)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let registry = DruidDriverRegistry::builtin().map_err(|error| error.to_string())?;
    let platform = format!("{}/{}", env::consts::OS, env::consts::ARCH);

    let mut rows = vec![
        "profile_id\tproduct_version\tfeature\tplatform\tcapability\tstatus\tdetail".to_owned(),
    ];
    let mut counts = std::collections::BTreeMap::new();
    for profile in registry.profiles() {
        let feature = profile.provider_id();
        let version = profile.artifact_version().unwrap_or("unverified");
        let profile_id = profile.id().to_string();
        if profile_id == "sqlite" {
            let config = DatabaseConnectionConfig::new("sqlite", "sqlite::memory:")
                .map_err(|error| error.to_string())?;
            let resolved = registry
                .resolve(&config)
                .map_err(|error| error.to_string())?;
            for (capability, status, detail) in runtime.block_on(run_contract(resolved.factory())) {
                *counts.entry(status).or_insert(0_usize) += 1;
                rows.push(format!(
                    "{profile_id}\t{version}\t{feature}\t{platform}\t{capability}\t{status}\t{detail}"
                ));
            }
        } else {
            for capability in OPERATIONS {
                *counts.entry("NOT_RUN").or_insert(0_usize) += 1;
                rows.push(format!(
                    "{profile_id}\t{version}\t{feature}\t{platform}\t{capability}\tNOT_RUN\tno configured runtime or credentials for this profile"
                ));
            }
        }
    }
    write_atomic(&output, &(rows.join("\n") + "\n"))?;
    println!(
        "DRIVER_MATRIX profiles={} platform={platform} statuses={counts:?} output={}",
        registry.catalog_size(),
        output.display()
    );
    Ok(())
}

async fn run_contract(
    factory: Arc<dyn druid_rdbc::driver::ConnectionFactory>,
) -> Vec<(&'static str, &'static str, String)> {
    let mut results: Vec<(&'static str, &'static str, String)> = OPERATIONS
        .iter()
        .map(|operation| (*operation, "NOT_RUN", "not reached".to_owned()))
        .collect();

    let mut connection = match factory.create().await {
        Ok(connection) => {
            set(&mut results, "connect", "PASS", String::new());
            connection
        }
        Err(error) => {
            let (status, detail) = classify(&error);
            set(&mut results, "connect", status, detail);
            return results;
        }
    };

    record(
        &mut results,
        "query",
        query_scalar(&mut *connection, "SELECT 1", 1).await,
    );
    record(
        &mut results,
        "update",
        async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            statement
                .execute_update(
                    "CREATE TABLE matrix_probe(id INTEGER PRIMARY KEY, value TEXT)",
                    GeneratedKeys::None,
                )
                .await?;
            statement
                .execute_update(
                    "INSERT INTO matrix_probe(id, value) VALUES (1, 'matrix')",
                    GeneratedKeys::None,
                )
                .await?;
            statement.close().await
        }
        .await,
    );
    record(
        &mut results,
        "prepared",
        async {
            let mut prepared = connection
                .prepare_statement("SELECT 1", StatementOptions::default())
                .await?;
            prepared.close().await
        }
        .await,
    );
    record(
        &mut results,
        "metadata",
        async {
            let _metadata = connection.database_meta_data().await?;
            Ok(())
        }
        .await,
    );
    record(
        &mut results,
        "types",
        async {
            query_scalar(&mut *connection, "SELECT 1 AS n, 'x' AS s", 1).await?;
            let mut rows = connection
                .fetch("SELECT 1 AS n, 'x' AS s", Vec::new())
                .await?;
            let first = rows
                .pop()
                .ok_or_else(|| RdbcError::Other("no row".to_owned()))?;
            if first.get(1).is_none() {
                return Err(RdbcError::Other("missing second column".to_owned()));
            }
            Ok(())
        }
        .await,
    );
    record(
        &mut results,
        "null",
        async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            let mut result_set = statement.execute_query("SELECT NULL AS n").await?;
            if !result_set.next().await? {
                return Err(RdbcError::Other("no row".to_owned()));
            }
            let index = ColumnIndex::new(1)?;
            let value = result_set.string(index)?;
            let is_null = result_set.was_null()?;
            result_set.close().await?;
            statement.close().await?;
            if value.is_none() && is_null {
                Ok(())
            } else {
                Err(RdbcError::Other("NULL was not reported as null".to_owned()))
            }
        }
        .await,
    );
    record(
        &mut results,
        "batch",
        async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            statement
                .add_batch("INSERT INTO matrix_probe(id, value) VALUES (2, 'b')".to_owned())?;
            statement
                .add_batch("INSERT INTO matrix_probe(id, value) VALUES (3, 'c')".to_owned())?;
            let counts = statement.execute_batch().await?;
            statement.close().await?;
            if counts.len() == 2 {
                Ok(())
            } else {
                Err(RdbcError::Other("unexpected batch count".to_owned()))
            }
        }
        .await,
    );
    record(
        &mut results,
        "transaction",
        async {
            connection.set_auto_commit(false).await?;
            connection.commit().await?;
            connection.rollback().await?;
            connection.set_auto_commit(true).await
        }
        .await,
    );
    record(
        &mut results,
        "savepoint",
        async {
            let savepoint = connection.set_savepoint().await?;
            connection.rollback_to(&savepoint).await
        }
        .await,
    );
    record(
        &mut results,
        "cancel",
        async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            let result = statement.cancel();
            statement.close().await?;
            result
        }
        .await,
    );
    record(
        &mut results,
        "error",
        async {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await?;
            let result = statement
                .execute_update("THIS IS NOT VALID SQL", GeneratedKeys::None)
                .await;
            statement.close().await?;
            match result {
                Err(_) => Ok(()),
                Ok(_) => Err(RdbcError::Other(
                    "invalid SQL unexpectedly succeeded".to_owned(),
                )),
            }
        }
        .await,
    );
    record(
        &mut results,
        "close",
        async {
            let result = connection.close().await;
            result
        }
        .await,
    );
    results
}

async fn query_scalar(
    connection: &mut dyn Connection,
    sql: &str,
    expected: i32,
) -> Result<(), RdbcError> {
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await?;
    let mut result_set = statement.execute_query(sql).await?;
    if !result_set.next().await? {
        return Err(RdbcError::Other("no row".to_owned()));
    }
    let index = ColumnIndex::new(1)?;
    let value = result_set.int(index)?;
    result_set.close().await?;
    statement.close().await?;
    if value == expected {
        Ok(())
    } else {
        Err(RdbcError::Other(format!(
            "expected {expected}, got {value}"
        )))
    }
}

fn record(
    results: &mut [(&'static str, &'static str, String)],
    operation: &'static str,
    result: Result<(), RdbcError>,
) {
    match result {
        Ok(()) => set(results, operation, "PASS", String::new()),
        Err(error) => {
            let (status, detail) = classify(&error);
            set(results, operation, status, detail);
        }
    }
}

fn set(
    results: &mut [(&'static str, &'static str, String)],
    operation: &'static str,
    status: &'static str,
    detail: String,
) {
    if let Some(row) = results
        .iter_mut()
        .find(|(candidate, _, _)| *candidate == operation)
    {
        row.1 = status;
        row.2 = detail;
    }
}

fn classify(error: &RdbcError) -> (&'static str, String) {
    if matches!(
        error.kind(),
        RdbcErrorKind::UnsupportedOperation | RdbcErrorKind::SqlFeatureNotSupported
    ) {
        ("UNSUPPORTED", error.to_string())
    } else {
        ("FAIL", error.to_string())
    }
}

fn arguments(root: &Path) -> Result<PathBuf, String> {
    let mut output =
        root.join("docs/verification/migration-completion-20261102/driver-matrix-results.tsv");
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} 缺少参数值"))?;
        match argument.as_str() {
            "--output" => output = PathBuf::from(value),
            other => return Err(format!("未知参数: {other}")),
        }
    }
    Ok(output)
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建 {} 失败: {error}", parent.display()))?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("写入 {} 失败: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| format!("替换 {} 失败: {error}", path.display()))
}
