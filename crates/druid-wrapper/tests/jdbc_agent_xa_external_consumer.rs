use druid_rdbc::data_source::{
    CommonDataSource, XaConnection, XaDataSource, XaError, XaFlags, Xid,
};
use druid_rdbc::error::RdbcError;
use druid_wrapper::jdbc_agent::{JdbcAgentOptions, JdbcAgentXaDataSource};
use serde_json::{json, Map};

fn assert_standard_xa_data_source<T: CommonDataSource + XaDataSource>() {}

async fn transaction_manager_starts_through_the_standard_resource(
    connection: &mut dyn XaConnection,
    xid: &Xid,
) -> Result<(), XaError> {
    connection
        .xa_resource()?
        .start(xid, XaFlags::TMNOFLAGS)
        .await
}

#[test]
fn public_xa_data_source_implements_only_the_rdbc_standard_contract() {
    assert_standard_xa_data_source::<JdbcAgentXaDataSource>();
    let _standard_resource_consumer = transaction_manager_starts_through_the_standard_resource;

    let source = JdbcAgentXaDataSource::new(
        "org.postgresql.xa.PGXADataSource",
        Map::new(),
        JdbcAgentOptions::new("external-xa-consumer"),
    )
    .expect("explicit XA class should construct");
    let source = source
        .property("serverName", json!("127.0.0.1"))
        .property("password", json!("consumer-secret"));

    let debug = format!("{source:?}");
    assert!(!debug.contains("consumer-secret"));
}

#[tokio::test]
async fn blank_xa_class_fails_before_agent_startup() {
    let error = match JdbcAgentXaDataSource::new(
        "  ",
        Map::new(),
        JdbcAgentOptions::new("external-xa-consumer"),
    ) {
        Ok(_) => panic!("blank XA class must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(error, RdbcError::InvalidArgument(_)));
}
