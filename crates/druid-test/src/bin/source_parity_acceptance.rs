//! 整仓来源用例验收聚合器。
//!
//! 读取 `docs/migration/source-test-parity.json`，校验每个来源用例的处置与
//! 证据一致，并生成逐用例状态表。命令不修改生产代码，也不把 `MISSING` 或
//! `NOT_RUN` 伪装成通过。
//!
//! 用法：
//! ```text
//! cargo run -p druid-test --bin source-parity-acceptance -- \
//!     [--manifest <path>] [--output <tsv>] [--summary <json>]
//! ```

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};

/// 已知的用例处置。
const DISPOSITIONS: [&str; 7] = [
    "MIRRORED",
    "ADAPTED",
    "SPLIT",
    "MERGED_APPROVED",
    "MISSING",
    "BLOCKED",
    "NOT_APPLICABLE",
];

/// 视为已迁移的处置。
const COMPLETED_DISPOSITIONS: [&str; 4] = ["MIRRORED", "ADAPTED", "SPLIT", "MERGED_APPROVED"];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("SOURCE_PARITY_ACCEPTANCE_FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = repository_root();
    let arguments = Arguments::parse(&root)?;
    let manifest_text = fs::read_to_string(&arguments.manifest)
        .map_err(|error| format!("读取 {} 失败: {error}", arguments.manifest.display()))?;
    let manifest: Value = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("解析 {} 失败: {error}", arguments.manifest.display()))?;

    let cases = manifest
        .get("source_tests")
        .and_then(Value::as_array)
        .ok_or_else(|| "source_tests 必须是数组".to_owned())?;
    if cases.is_empty() {
        return Err("source_tests 不能为空".to_owned());
    }
    let assets = manifest
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| "assets 必须是数组".to_owned())?;

    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut dispositions: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = Vec::with_capacity(cases.len());
    let mut seen = std::collections::BTreeSet::new();

    for (index, case) in cases.iter().enumerate() {
        let case_id = case
            .get("case_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("第 {index} 个来源用例缺少 case_id"))?;
        if !seen.insert(case_id.to_owned()) {
            return Err(format!("重复的 case_id: {case_id}"));
        }
        let disposition = case
            .get("disposition")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{case_id} 缺少 disposition"))?;
        if !DISPOSITIONS.contains(&disposition) {
            return Err(format!("{case_id} 使用未知 disposition: {disposition}"));
        }
        *dispositions.entry(disposition.to_owned()).or_default() += 1;
        let result_parity = case
            .get("result_parity")
            .and_then(Value::as_str)
            .unwrap_or("NOT_RUN");
        let completed = COMPLETED_DISPOSITIONS.contains(&disposition);
        let targets = case
            .get("targets")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let evidence = case.get("evidence").and_then(Value::as_str).unwrap_or("");

        // 已迁移处置必须有真实目标与证据；未迁移处置不得声称已运行。
        if completed && (targets.is_empty() || evidence.trim().is_empty()) {
            return Err(format!("{case_id} 已迁移但缺少 targets/evidence"));
        }
        if !completed && result_parity != "NOT_RUN" {
            return Err(format!(
                "{case_id} 处置为 {disposition}，却记录 result_parity={result_parity}"
            ));
        }

        let status = match result_parity {
            "MATCH" => "MATCH",
            "MISMATCH" => "MISMATCH",
            _ => "NOT_RUN",
        };
        *counts.entry(status).or_default() += 1;
        let target = targets
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(";");
        rows.push(format!(
            "{case_id}\t{disposition}\t{status}\t{target}\t{}",
            evidence.replace(['\t', '\n', '\r'], " ")
        ));
    }

    if let Some(output) = &arguments.output {
        write_atomic(
            output,
            &format!(
                "case_id\tdisposition\tstatus\ttargets\tevidence\n{}\n",
                rows.join("\n")
            ),
        )?;
    }

    let summary = json!({
        "schema": manifest.get("schema"),
        "java_baseline": manifest.get("java_baseline"),
        "rust_baseline": manifest.get("rust_baseline"),
        "cases": cases.len(),
        "assets": assets.len(),
        "dispositions": dispositions,
        "statuses": counts,
        "acceptance_module": manifest.get("acceptance_module"),
        "manifest": relative(&root, &arguments.manifest),
        "status_table": arguments.output.as_ref().map(|path| relative(&root, path)),
    });
    if let Some(path) = &arguments.summary {
        write_atomic(
            path,
            &serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())?,
        )?;
    }
    println!(
        "SOURCE_PARITY_ACCEPTANCE cases={} assets={} match={} mismatch={} not_run={}",
        cases.len(),
        assets.len(),
        counts.get("MATCH").copied().unwrap_or(0),
        counts.get("MISMATCH").copied().unwrap_or(0),
        counts.get("NOT_RUN").copied().unwrap_or(0),
    );
    Ok(())
}

/// 命令行参数。
struct Arguments {
    manifest: PathBuf,
    output: Option<PathBuf>,
    summary: Option<PathBuf>,
}

impl Arguments {
    fn parse(root: &Path) -> Result<Self, String> {
        let mut manifest = root.join("docs/migration/source-test-parity.json");
        let mut output = Some(
            root.join("docs/verification/migration-completion-20261102/source-parity-status.tsv"),
        );
        let mut summary = Some(
            root.join("docs/verification/migration-completion-20261102/source-parity-summary.json"),
        );
        let mut arguments = env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("{argument} 缺少参数值"))?;
            match argument.as_str() {
                "--manifest" => manifest = PathBuf::from(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--summary" => summary = Some(PathBuf::from(value)),
                other => return Err(format!("未知参数: {other}")),
            }
        }
        Ok(Self {
            manifest,
            output,
            summary,
        })
    }
}

/// 仓库根目录：`CARGO_MANIFEST_DIR` 的上一级。
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate 目录位于仓库根目录下")
        .to_path_buf()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
