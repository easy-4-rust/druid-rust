use druid::core::ConnectionFactory;
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{CommonDataSource as StandardCommonDataSource, RdbcLogWriter};
use druid_rdbc::error::RdbcError;
#[cfg(feature = "jdbc-agent")]
use druid_wrapper::jdbc_agent::{JdbcAgentOptions, JdbcAgentXaDataSource};
use druid_wrapper::rdbc::DruidRdbcDataSource;
use parking_lot::Mutex;
#[cfg(feature = "jdbc-agent")]
use serde_json::Map;
use std::sync::Arc;
use std::time::Duration;

struct DocumentationFactory;

#[async_trait::async_trait]
impl ConnectionFactory for DocumentationFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::ConnectionDiscarded)
    }
}

fn example_writer() -> RdbcLogWriter {
    Arc::new(Mutex::new(Box::new(std::io::sink())))
}

async fn build_pool_source(login_timeout: i32) -> druid::pool::DruidDataSource {
    DruidPoolBuilder::new()
        .name("common-data-source-docs")
        .driver_name("documentation-factory")
        .factory(Arc::new(DocumentationFactory))
        .login_timeout(login_timeout)
        .build_data_source()
        .await
        .expect("build pool without opening a physical connection")
}

#[tokio::test]
async fn common_data_source_configuration_examples_use_public_contracts() {
    // 池配置示例：建池前用 Java 对应秒数配置物理登录超时。
    let pooled = build_pool_source(37).await;

    assert_eq!(
        StandardCommonDataSource::try_login_timeout(&pooled).expect("read pool timeout"),
        Some(Duration::from_secs(37))
    );
    let writer = example_writer();
    StandardCommonDataSource::set_log_writer(&pooled, Some(Arc::clone(&writer)))
        .expect("configure pool diagnostics");
    assert!(Arc::ptr_eq(
        &writer,
        &StandardCommonDataSource::get_log_writer(&pooled).expect("read pool writer")
    ));

    // 普通数据源示例：标准 wrapper 委托同一池配置，不创建第二层池。
    let mut ordinary = DruidRdbcDataSource::new(pooled);
    assert_eq!(
        StandardCommonDataSource::try_login_timeout(&ordinary).expect("read wrapped pool timeout"),
        Some(Duration::from_secs(37))
    );
    StandardCommonDataSource::set_log_writer(&ordinary, Some(Arc::clone(&writer)))
        .expect("configure ordinary diagnostics");
    assert!(Arc::ptr_eq(
        &writer,
        &StandardCommonDataSource::get_log_writer(&ordinary).expect("read ordinary writer")
    ));

    // 已构建池不能只改变显示值；标准 setter 必须保留不可变配置错误。
    assert!(
        StandardCommonDataSource::set_login_timeout(&mut ordinary, Duration::from_secs(5)).is_err()
    );
    assert_eq!(
        StandardCommonDataSource::try_login_timeout(&ordinary).expect("timeout remains intact"),
        Some(Duration::from_secs(37))
    );

    let default_timeout = build_pool_source(0).await;
    assert_eq!(
        StandardCommonDataSource::try_login_timeout(&default_timeout)
            .expect("zero means no login wait limit"),
        Some(Duration::ZERO)
    );
    let negative_timeout = build_pool_source(-1).await;
    assert!(matches!(
        StandardCommonDataSource::try_login_timeout(&negative_timeout),
        Err(RdbcError::InvalidArgument(_))
    ));

    #[cfg(feature = "jdbc-agent")]
    {
        // XA 示例：显式构造厂商 XA 数据源；厂商属性在创建时提供。
        let xa_options = JdbcAgentOptions::new("java");
        let xa_source =
            JdbcAgentXaDataSource::new("org.example.DriverXaDataSource", Map::new(), xa_options)
                .expect("construct explicit XA source");
        StandardCommonDataSource::set_log_writer(&xa_source, Some(example_writer()))
            .expect("configure XA diagnostics");
        assert!(StandardCommonDataSource::get_log_writer(&xa_source).is_some());
        assert_eq!(
            StandardCommonDataSource::get_parent_logger(&xa_source),
            Ok("druid.jdbc_agent.xa")
        );
    }
}
