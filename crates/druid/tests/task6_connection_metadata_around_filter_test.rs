use druid::core::{BeforeFilter, DruidError, ExecContext, FilterChain, RdbcError};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::{Arc, Mutex};

struct SyntheticMetadata;
druid_rdbc::impl_standard_wrapper!(SyntheticMetadata => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for SyntheticMetadata {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        Ok("filtered-product".to_owned())
    }
}

struct MetadataAroundFilter {
    name: &'static str,
    calls: Arc<Mutex<Vec<String>>>,
    short_circuit: bool,
}

struct LegacyMetadataObserver {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for LegacyMetadataObserver {
    fn name(&self) -> &str {
        "legacy"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn standard_connection_get_meta_data_before(
        &self,
        _connection_id: u64,
    ) -> Result<(), DruidError> {
        self.calls.lock().unwrap().push("legacy:before".to_owned());
        Ok(())
    }

    async fn standard_connection_get_meta_data_after(
        &self,
        _connection_id: u64,
        result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        self.calls.lock().unwrap().push(if result.is_ok() {
            "legacy:after:ok".to_owned()
        } else {
            "legacy:after:error".to_owned()
        });
        Ok(())
    }
}

struct RejectingMetadataFilter;

#[async_trait::async_trait]
impl BeforeFilter for RejectingMetadataFilter {
    fn name(&self) -> &str {
        "reject"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn standard_connection_get_meta_data(
        &self,
        _chain: &mut druid::core::StandardConnectionMetaDataFilterChain<'_>,
    ) -> Result<Box<dyn DatabaseMetaData>, DruidError> {
        Err(DruidError::from(RdbcError::DriverError(
            "metadata rejected".to_owned(),
        )))
    }
}

#[async_trait::async_trait]
impl BeforeFilter for MetadataAroundFilter {
    fn name(&self) -> &str {
        self.name
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn standard_connection_get_meta_data(
        &self,
        chain: &mut druid::core::StandardConnectionMetaDataFilterChain<'_>,
    ) -> Result<Box<dyn DatabaseMetaData>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = if self.short_circuit {
            Ok(Box::new(SyntheticMetadata) as Box<dyn DatabaseMetaData>)
        } else {
            chain.connection_get_meta_data().await
        };
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:after", self.name));
        result
    }
}

#[tokio::test]
async fn connection_metadata_filters_can_replace_result_and_short_circuit_driver() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_before(Arc::new(MetadataAroundFilter {
            name,
            calls: Arc::clone(&calls),
            short_circuit: name == "B",
        }));
    }
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPoolBuilder::new()
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(filters))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_eq!(
        metadata.database_product_name().await.unwrap(),
        "filtered-product"
    );
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:after", "A:after"]
    );
    connection.close().await.unwrap();
    assert!(matches!(
        metadata.database_product_name().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    pool.close().await;
}

#[tokio::test]
async fn connection_metadata_default_around_preserves_legacy_observer_and_error_unwind() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_before(Arc::new(LegacyMetadataObserver {
        calls: Arc::clone(&calls),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPoolBuilder::new()
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(filters))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_ne!(
        metadata.database_product_name().await.unwrap(),
        "filtered-product"
    );
    assert_eq!(*calls.lock().unwrap(), ["legacy:before", "legacy:after:ok"]);
    connection.close().await.unwrap();
    pool.close().await;

    calls.lock().unwrap().clear();
    let mut filters = FilterChain::new();
    filters.add_before(Arc::new(LegacyMetadataObserver {
        calls: Arc::clone(&calls),
    }));
    filters.add_before(Arc::new(RejectingMetadataFilter));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPoolBuilder::new()
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(filters))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    assert!(matches!(
        connection.database_meta_data().await,
        Err(RdbcError::DriverError(message)) if message == "metadata rejected"
    ));
    assert_eq!(
        *calls.lock().unwrap(),
        ["legacy:before", "legacy:after:error"]
    );
    connection.close().await.unwrap();
    pool.close().await;
}
