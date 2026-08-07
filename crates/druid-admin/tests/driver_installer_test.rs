#![cfg(feature = "managed-driver-install")]

use druid_admin::driver::{
    DriverBundleFile, DriverBundleInstallRequest, DriverInstallRequest, DriverInstaller,
    DriverInstallerError,
};
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

#[tokio::test]
async fn installer_supports_version_rollback_verification_and_safe_removal() {
    let root = std::env::temp_dir().join(format!(
        "druid-driver-version-test-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let first = root.join("h2-first.jar");
    let second = root.join("h2-second.jar");
    fs::write(&first, b"PK\x03\x04first-version").expect("必须创建第一版 JAR");
    fs::write(&second, b"PK\x03\x04second-version").expect("必须创建第二版 JAR");
    let installer = DriverInstaller::new(root.join("managed"));

    let first = installer
        .install_file(&DriverInstallRequest::new("h2", &first))
        .await
        .expect("必须安装第一版");
    let second = installer
        .install_file(&DriverInstallRequest::new("h2", &second))
        .await
        .expect("必须安装第二版");
    assert_eq!(installer.installations("h2").await.unwrap().len(), 2);
    assert_eq!(
        installer.active_installation("h2").await.unwrap().sha256(),
        second.sha256()
    );
    installer
        .verify_installation("h2", first.sha256())
        .await
        .expect("旧版本仍必须可校验");

    let rolled_back = installer
        .activate_version("h2", first.sha256())
        .await
        .expect("必须只切换激活记录完成回滚");
    assert_eq!(rolled_back.sha256(), first.sha256());
    assert!(matches!(
        installer.remove_version("h2", first.sha256()).await,
        Err(DriverInstallerError::ActiveArtifact { .. })
    ));
    installer
        .remove_version("h2", second.sha256())
        .await
        .expect("非激活版本必须可删除");
    assert_eq!(installer.installations("h2").await.unwrap().len(), 1);
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[tokio::test]
async fn installer_keeps_multi_jar_bundle_atomic_and_detects_dependency_tampering() {
    let root =
        std::env::temp_dir().join(format!("druid-driver-bundle-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let driver = root.join("h2-driver.jar");
    let dependency = root.join("h2-dependency.jar");
    fs::write(&driver, b"PK\x03\x04bundle-driver").expect("必须创建主驱动 JAR");
    fs::write(&dependency, b"PK\x03\x04bundle-dependency").expect("必须创建依赖 JAR");
    let installer = DriverInstaller::new(root.join("managed"));
    let request = DriverBundleInstallRequest::new(
        "h2",
        vec![
            DriverBundleFile::new(&driver),
            DriverBundleFile::new(&dependency),
        ],
    );
    let installed = installer
        .install_bundle_files(&request)
        .await
        .expect("多 JAR bundle 必须原子安装");
    assert!(installed.is_bundle());
    assert_eq!(installed.class_path().len(), 2);
    assert_eq!(installed.jar_sha256().len(), 2);
    installer
        .verify_installation("h2", installed.sha256())
        .await
        .expect("完整 bundle 必须可复验");

    fs::write(&installed.class_path()[1], b"PK\x03\x04tampered-dependency")
        .expect("必须能模拟依赖篡改");
    assert!(matches!(
        installer
            .verify_installation("h2", installed.sha256())
            .await,
        Err(DriverInstallerError::ChecksumMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[tokio::test]
async fn concurrent_installation_is_locked_and_runtime_identity_is_content_based() {
    let root =
        std::env::temp_dir().join(format!("druid-driver-lock-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let driver_source = root.join("h2.jar");
    let agent_source = root.join("agent.jar");
    fs::write(&driver_source, b"PK\x03\x04driver").expect("必须创建驱动 JAR");
    fs::write(&agent_source, b"PK\x03\x04agent").expect("必须创建 Agent JAR");
    let installer = DriverInstaller::new(root.join("managed"));
    let first_installer = installer.clone();
    let second_installer = installer.clone();
    let first_request = DriverInstallRequest::new("h2", &driver_source);
    let second_request = first_request.clone();
    let (first, second) = tokio::join!(
        first_installer.install_file(&first_request),
        second_installer.install_file(&second_request)
    );
    assert_eq!(first.unwrap().sha256(), second.unwrap().sha256());

    let agent = installer
        .install_agent_file(&agent_source, None)
        .await
        .expect("必须安装 Agent");
    let driver = installer.active_installation("h2").await.unwrap();
    let options = installer
        .jdbc_agent_options("h2", "java")
        .await
        .expect("必须从固定制品生成启动配置");
    assert_eq!(
        options.agent_key(),
        format!("jdbc-agent:{}", agent.sha256())
    );
    assert_eq!(
        options.artifact_version(),
        format!("agent={};driver={}", agent.sha256(), driver.sha256())
    );
    assert_eq!(options.jvm_options_hash().len(), 64);
    assert_eq!(driver.artifact_version(), driver.sha256());
    assert_eq!(driver.license(), "NOASSERTION");
    assert_eq!(driver.jar_files(), &["h2.jar".to_owned()]);
    assert_eq!(driver.minimum_java_version(), 17);

    let next_driver_source = root.join("h2-next.jar");
    fs::write(&next_driver_source, b"PK\x03\x04next-driver").expect("必须创建下一版驱动");
    installer
        .install_file(&DriverInstallRequest::new("h2", &next_driver_source))
        .await
        .expect("必须激活下一版驱动");
    assert!(matches!(
        installer.remove_version("h2", driver.sha256()).await,
        Err(DriverInstallerError::ActiveArtifact { .. })
    ));
    drop(options);
    installer
        .remove_version("h2", driver.sha256())
        .await
        .expect("Agent 运行时配置释放后旧版本必须可删除");
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[tokio::test]
async fn offline_installer_rejects_network_before_url_processing() {
    let installer = DriverInstaller::new(std::env::temp_dir().join(format!(
        "druid-driver-offline-test-{}",
        uuid::Uuid::new_v4()
    )))
    .offline(true);
    assert!(matches!(
        installer
            .install_url("h2", "not-even-a-url", "h2.jar", &"0".repeat(64))
            .await,
        Err(DriverInstallerError::OfflineMode(_))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn installer_rejects_symbolic_link_sources() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "druid-driver-symlink-test-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).expect("必须创建测试目录");
    let target = root.join("target.jar");
    let link = root.join("link.jar");
    fs::write(&target, b"PK\x03\x04driver").expect("必须创建目标 JAR");
    symlink(&target, &link).expect("必须创建符号链接");
    let installer = DriverInstaller::new(root.join("managed"));
    assert!(matches!(
        installer
            .install_file(&DriverInstallRequest::new("h2", &link))
            .await,
        Err(DriverInstallerError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(root).expect("必须清理测试目录");
}

#[cfg(unix)]
#[tokio::test]
async fn installer_registers_and_revalidates_a_java_17_runtime() {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    use std::os::unix::fs::PermissionsExt;

    let root =
        std::env::temp_dir().join(format!("druid-java-runtime-test-{}", uuid::Uuid::new_v4()));
    let java_home = root.join("runtime");
    let java_program = java_home.join("bin").join("java");
    fs::create_dir_all(java_program.parent().unwrap()).expect("必须创建模拟 JRE");
    fs::write(
        &java_program,
        b"#!/bin/sh\nprintf 'openjdk version \"17.0.12\"\\n' >&2\n",
    )
    .expect("必须创建模拟 Java 程序");
    let mut permissions = fs::metadata(&java_program).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&java_program, permissions).expect("必须允许执行模拟 Java");
    let mut sha256 = String::new();
    for byte in Sha256::digest(fs::read(&java_program).unwrap()) {
        write!(&mut sha256, "{byte:02x}").unwrap();
    }
    let installer = DriverInstaller::new(root.join("managed"));
    let runtime = installer
        .register_java_runtime(&java_home, &sha256)
        .await
        .expect("必须登记已校验 Java 17 runtime");
    assert_eq!(runtime.major_version(), 17);
    assert_eq!(runtime.sha256(), sha256);
    assert_eq!(installer.active_java_runtime().await.unwrap(), runtime);

    fs::write(&java_program, b"tampered").expect("必须能模拟 JRE 被篡改");
    assert!(matches!(
        installer.active_java_runtime().await,
        Err(DriverInstallerError::ChecksumMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("必须清理测试目录");
}
