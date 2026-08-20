#![allow(unexpected_cfgs)]
#![cfg(feature = "managed-driver-install")]

use druid_admin::driver::{DriverInstaller, DriverRuntimeDiagnostics};

#[tokio::test]
async fn diagnostics_distinguishes_local_readiness_from_live_database_support() {
    let root = std::env::temp_dir().join(format!(
        "druid-driver-diagnostics-test-{}",
        uuid::Uuid::new_v4()
    ));
    let report = DriverRuntimeDiagnostics::new(DriverInstaller::new(&root))
        .check("h2")
        .await
        .expect("H2 档案必须可以执行本地就绪度诊断");
    let report_json = serde_json::to_value(&report).expect("诊断报告必须可序列化");

    assert!(!report.ready());
    assert_eq!(report_json["profileId"], "h2");
    assert_eq!(report_json["catalogStatus"], "Declared");
    assert_eq!(report_json["agentInstalled"], false);
    assert_eq!(report_json["driverInstalled"], false);
    assert!(report_json["messages"]
        .as_array()
        .is_some_and(|messages| !messages.is_empty()));
}
