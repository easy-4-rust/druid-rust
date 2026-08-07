use druid_admin::driver::{DriverInstallRequest, DriverInstaller, DriverInstallerError};
use std::fs;

#[tokio::test]
async fn installer_uses_content_addressing_and_detects_tampering() {
    let root = std::env::temp_dir().join(format!(
        "druid-driver-installer-test-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let source = root.join("h2-test.jar");
    fs::write(&source, b"PK\x03\x04druid-driver-test").expect("必须创建测试 JAR");
    let installer = DriverInstaller::new(root.join("managed"));

    let installed_driver = installer
        .install_file(&DriverInstallRequest::new("h2", &source))
        .await
        .expect("H2 JDBC 档案必须允许显式安装");
    assert!(installed_driver.path().is_file());
    assert_eq!(installed_driver.sha256().len(), 64);
    assert_eq!(
        installer
            .active_installation("h2")
            .await
            .expect("必须解析激活工件"),
        installed_driver
    );

    fs::write(installed_driver.path(), b"PK\x03\x04tampered").expect("必须能模拟篡改");
    assert!(matches!(
        installer.active_installation("h2").await,
        Err(DriverInstallerError::ChecksumMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[tokio::test]
async fn installer_rejects_non_jdbc_catalog_profiles() {
    let root = std::env::temp_dir().join(format!(
        "druid-driver-profile-test-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let source = root.join("sqlite.jar");
    fs::write(&source, b"PK\x03\x04not-used").expect("必须创建测试 JAR");
    let installer = DriverInstaller::new(root.join("managed"));

    assert!(matches!(
        installer
            .install_file(&DriverInstallRequest::new("sqlite", &source))
            .await,
        Err(DriverInstallerError::NotJdbcProfile(_))
    ));
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[tokio::test]
async fn remote_install_rejects_non_https_before_network_access() {
    let installer = DriverInstaller::new(
        std::env::temp_dir().join(format!("druid-driver-https-test-{}", uuid::Uuid::new_v4())),
    );
    let result = installer
        .install_url(
            "h2",
            "http://127.0.0.1/not-allowed.jar",
            "h2.jar",
            &"0".repeat(64),
        )
        .await;

    assert!(matches!(result, Err(DriverInstallerError::InvalidUrl(_))));
}
