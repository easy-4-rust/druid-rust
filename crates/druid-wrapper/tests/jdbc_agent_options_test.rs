#![cfg(feature = "jdbc-agent")]

use druid_wrapper::jdbc_agent::{JdbcAgentOptions, JdbcAgentOptionsError};
use std::ffi::OsString;
use std::path::PathBuf;

#[test]
fn managed_java_options_accept_only_the_jvm_allowlist() {
    let options = JdbcAgentOptions::java_with_jvm_options(
        "java",
        [PathBuf::from("agent.jar"), PathBuf::from("driver.jar")],
        [
            OsString::from("-Xms64m"),
            OsString::from("-Xmx512m"),
            OsString::from("-XX:MaxRAMPercentage=75"),
            OsString::from("-Dfile.encoding=UTF-8"),
            OsString::from("-Duser.timezone=Asia/Shanghai"),
        ],
    )
    .expect("allowlist 参数必须可用");
    assert_eq!(options.jvm_options_hash().len(), 64);

    for unsafe_option in [
        "-javaagent:evil.jar",
        "-agentlib:jdwp=transport=dt_socket,server=y,address=*:5005",
        "-cp",
        "--module-path=evil",
        "com.example.EvilMain",
    ] {
        assert!(matches!(
            JdbcAgentOptions::java_with_jvm_options(
                "java",
                [PathBuf::from("agent.jar")],
                [OsString::from(unsafe_option)],
            ),
            Err(JdbcAgentOptionsError::UnsafeJvmOption(_))
        ));
    }
}
