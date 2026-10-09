#![cfg(feature = "managed-driver-install")]

use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_druid-driver"))
        .args(arguments)
        .output()
        .expect("必须启动 Wrapper 驱动管理入口")
}

#[test]
fn help_exposes_full_management_contract() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("帮助必须为 UTF-8");
    for command in [
        "catalog",
        "list",
        "install-agent",
        "install-jre",
        "verify-jre",
        "install",
        "install-bundle",
        "update",
        "verify",
        "activate",
        "remove",
        "doctor",
        "aggregate-evidence",
    ] {
        assert!(text.contains(command), "缺失命令：{command}");
    }
}

#[test]
fn catalog_reports_profiles_and_truthful_support_count() {
    let output = run(&["catalog"]);
    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("目录必须输出 JSON");
    let profiles = json["profiles"].as_array().expect("必须包含产品列表");
    assert_eq!(profiles.len(), 80);
    assert_eq!(json["catalogSize"], 80);
    assert_eq!(
        json["supportedCount"].as_u64().unwrap(),
        profiles
            .iter()
            .filter(|profile| profile["countsAsSupported"] == true)
            .count() as u64
    );
    assert!(profiles.iter().any(|profile| profile["id"] == "h2"));
}

#[test]
fn missing_argument_and_unknown_command_fail_explicitly() {
    for (arguments, message) in [
        (vec!["install-agent"], "missing required argument <root>"),
        (vec!["unknown-command"], "unknown command 'unknown-command'"),
    ] {
        let output = run(&arguments);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
}

#[test]
fn invalid_evidence_mode_fails_before_writing_output() {
    let output = run(&[
        "aggregate-evidence",
        "sqlite",
        "invalid-mode",
        ".",
        "unused.json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown runtime mode"));
}
