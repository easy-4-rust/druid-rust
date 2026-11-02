//! 生成 80 个驱动产品档案的支持基线。
//!
//! 只读取内置 `DruidDriverRegistry`，不声称任何产品已通过真实数据库验证；
//! `verified`/`certified` 只统计清单中的证据状态。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use druid_wrapper::driver::DruidDriverRegistry;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("DRIVER_PROFILE_BASELINE_FAIL: {error}");
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
    let registry = DruidDriverRegistry::builtin().map_err(|error| error.to_string())?;
    let platform = format!("{}/{}", env::consts::OS, env::consts::ARCH);

    let mut rows = Vec::new();
    rows.push(
        "profile_id\tdisplay_name\tsupport_status\truntime_mode\tprovider_id\tprotocol_family\tartifact_id\tartifact_version\tplatform\tcounts_as_supported"
            .to_owned(),
    );
    let mut declared = 0;
    let mut experimental = 0;
    let mut supported = 0;
    for profile in registry.profiles() {
        let status = format!("{:?}", profile.support_status()).to_lowercase();
        if status == "declared" {
            declared += 1;
        }
        if status == "experimental" {
            experimental += 1;
        }
        let counts = profile.support_status().counts_as_supported();
        if counts {
            supported += 1;
        }
        rows.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            profile.id(),
            profile.display_name(),
            status,
            format!("{:?}", profile.runtime_mode()).to_lowercase(),
            profile.provider_id(),
            format!("{:?}", profile.protocol_family()).to_lowercase(),
            profile.artifact_id(),
            profile.artifact_version().unwrap_or("-"),
            platform,
            counts,
        ));
    }
    let total = registry.catalog_size();
    write_atomic(&output, &(rows.join("\n") + "\n"))?;
    println!(
        "DRIVER_PROFILE_BASELINE profiles={total} declared={declared} experimental={experimental} supported={supported} platform={platform} output={}",
        output.display()
    );
    Ok(())
}

fn arguments(root: &Path) -> Result<PathBuf, String> {
    let mut output =
        root.join("docs/verification/migration-completion-20261102/driver-profiles-baseline.tsv");
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
