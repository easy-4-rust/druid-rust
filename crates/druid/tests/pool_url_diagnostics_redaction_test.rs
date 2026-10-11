//! 管理统计与周期快照不得将连接 URL 中的凭据写入诊断输出。

use druid::core::{Connection, ConnectionFactory, RdbcError};
use druid::pool::{DruidDataSource, DruidPool, PoolInnerConfig};
use druid::stats::DataSourceMonitorable;
use std::sync::Arc;

struct CredentialUrlFactory;

#[async_trait::async_trait]
impl ConnectionFactory for CredentialUrlFactory {
    fn connection_url(&self) -> Option<&str> {
        Some("postgres://alice:factory_secret@db.example/app?token=factory_token")
    }

    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        panic!("diagnostic snapshots must not establish a connection")
    }
}

#[test]
fn admin_and_interval_stats_hide_connection_credentials() {
    let config = PoolInnerConfig {
        url: Some("postgres://alice:config_secret@db.example/app?token=config_token".into()),
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new(
        "credentials".into(),
        "test".into(),
        Arc::new(CredentialUrlFactory),
        config,
        None,
    );
    let data_source = DruidDataSource::from_pool(pool);

    let admin = data_source.data_source_stat_data().to_string();
    let interval = serde_json::to_string(&data_source.stat_value_and_reset()).unwrap();
    for output in [&admin, &interval] {
        for secret in [
            "config_secret",
            "config_token",
            "factory_secret",
            "factory_token",
        ] {
            assert!(
                !output.contains(secret),
                "diagnostic leaked {secret}: {output}"
            );
        }
    }
}
