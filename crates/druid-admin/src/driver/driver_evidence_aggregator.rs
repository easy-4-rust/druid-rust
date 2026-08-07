use super::DriverEvidenceError;
use druid_wrapper::driver::{DriverRuntimeMode, DriverVerificationEvidence};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

/// 将五目标、Rust 1.95/default 的逐次运行记录聚合为可进入 manifest 的证据。
pub struct DriverEvidenceAggregator;

impl DriverEvidenceAggregator {
    /// 读取目录中的 JSON 运行记录并构建已自校验的证据对象。
    pub fn aggregate(
        directory: impl AsRef<Path>,
        profile_id: &str,
        runtime_mode: DriverRuntimeMode,
    ) -> Result<Value, DriverEvidenceError> {
        let mut runs = Vec::new();
        let mut platforms = BTreeSet::new();
        let mut java_versions = BTreeSet::new();
        let mut artifact_sha256 = None::<String>;
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
            {
                continue;
            }
            let mut value: Value = serde_json::from_slice(&std::fs::read(entry.path())?)?;
            let object = value.as_object_mut().ok_or_else(|| {
                DriverEvidenceError::Incomplete("run record must be a JSON object".to_owned())
            })?;
            let actual_profile = object
                .remove("profileId")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| DriverEvidenceError::Incomplete("run lacks profileId".to_owned()))?;
            if actual_profile != profile_id {
                return Err(DriverEvidenceError::Incomplete(format!(
                    "run profile '{actual_profile}' does not match '{profile_id}'"
                )));
            }
            let target = object
                .get("target")
                .and_then(Value::as_str)
                .ok_or_else(|| DriverEvidenceError::Incomplete("run lacks target".to_owned()))?;
            platforms.insert(platform_for_target(target)?.to_owned());
            if let Some(versions) = object.get("javaVersions").and_then(Value::as_array) {
                for version in versions {
                    if let Some(version) =
                        version.as_u64().and_then(|value| u16::try_from(value).ok())
                    {
                        java_versions.insert(version);
                    }
                }
            }
            if let Some(checksum) = object.get("artifactSha256").and_then(Value::as_str) {
                match &artifact_sha256 {
                    Some(expected) if expected != checksum => {
                        return Err(DriverEvidenceError::Incomplete(
                            "runs reference different artifact SHA-256 values".to_owned(),
                        ));
                    }
                    None => artifact_sha256 = Some(checksum.to_owned()),
                    _ => {}
                }
            }
            runs.push(Value::Object(std::mem::take(object)));
        }
        if runs.is_empty() {
            return Err(DriverEvidenceError::Incomplete(
                "no evidence run JSON files were found".to_owned(),
            ));
        }
        runs.sort_by(|left, right| run_key(left).cmp(&run_key(right)));
        let evidence = json!({
            "contractVersion": "druid-database-contract-v1",
            "testedAt": chrono::Utc::now().to_rfc3339(),
            "platforms": platforms,
            "javaVersions": java_versions,
            "artifactSha256": artifact_sha256,
            "runs": runs,
        });
        let parsed: DriverVerificationEvidence = serde_json::from_value(evidence.clone())?;
        if !parsed.is_valid_for(runtime_mode) {
            return Err(DriverEvidenceError::Incomplete(
                "five targets, Rust 1.95/default, runtime paths or contract checks are incomplete"
                    .to_owned(),
            ));
        }
        Ok(evidence)
    }
}

fn platform_for_target(target: &str) -> Result<&'static str, DriverEvidenceError> {
    if target.contains("linux") {
        Ok("linux")
    } else if target.contains("apple-darwin") {
        Ok("macos")
    } else if target.contains("windows") {
        Ok("windows")
    } else {
        Err(DriverEvidenceError::Incomplete(format!(
            "unsupported evidence target '{target}'"
        )))
    }
}

fn run_key(value: &Value) -> (String, String) {
    (
        value
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        value
            .get("rustVersion")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKS: [&str; 13] = [
        "connection-lifecycle",
        "validation",
        "crud-ddl",
        "scalar-types",
        "prepared-and-batch",
        "transactions",
        "state-reset",
        "capabilities",
        "error-classification",
        "timeout-cancel",
        "database-restart",
        "concurrency-leak-shutdown",
        "no-pool-in-pool",
    ];

    #[test]
    fn aggregates_exact_five_target_dual_toolchain_contract() {
        let directory = std::env::temp_dir().join(format!(
            "druid-evidence-aggregator-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("必须创建证据测试目录");
        let targets = [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ];
        for target in targets {
            for rust_version in ["1.95", "default"] {
                let record = json!({
                    "profileId": "sqlite",
                    "target": target,
                    "databaseVersion": "3.50.0",
                    "rustVersion": rust_version,
                    "javaVersions": [],
                    "runtimeMode": "sqlx",
                    "installationPaths": ["native"],
                    "contractChecks": CHECKS,
                    "sourceRevision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "evidenceRef": "https://github.com/easy-4-rust/druid-rust/actions/runs/1",
                    "passedAt": "2026-08-07T00:00:00Z",
                    "artifactSha256": null
                });
                let path = directory.join(format!("{target}-{rust_version}.json"));
                std::fs::write(path, record.to_string()).expect("必须写入逐次证据");
            }
        }

        let evidence =
            DriverEvidenceAggregator::aggregate(&directory, "sqlite", DriverRuntimeMode::Sqlx)
                .expect("完整十次运行必须聚合成功");
        let parsed: DriverVerificationEvidence =
            serde_json::from_value(evidence).expect("聚合结果必须符合 manifest 证据模型");
        assert!(parsed.is_valid_for(DriverRuntimeMode::Sqlx));
        assert_eq!(parsed.runs().len(), 10);
        std::fs::remove_dir_all(directory).expect("必须清理证据测试目录");
    }

    #[test]
    fn rejects_incomplete_evidence_directory() {
        let directory = std::env::temp_dir().join(format!(
            "druid-evidence-incomplete-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("必须创建证据测试目录");
        assert!(matches!(
            DriverEvidenceAggregator::aggregate(&directory, "sqlite", DriverRuntimeMode::Sqlx),
            Err(DriverEvidenceError::Incomplete(_))
        ));
        std::fs::remove_dir_all(directory).expect("必须清理证据测试目录");
    }
}
