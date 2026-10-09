use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{DatabaseMetaData, ParameterMetaData, ResultSetMetaData, TablesQuery};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{PreparedStatement, Statement, StatementOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct ActorHandle(Arc<AtomicUsize>);

struct MockConnection(ActorHandle);
struct MockDatabaseMetaData(ActorHandle);
struct MockResultSet {
    actor: ActorHandle,
    meta_data: Arc<dyn ResultSetMetaData>,
    advanced: bool,
}
#[derive(Debug)]
struct MockResultSetMetaData;
struct MockParameterMetaData;
struct MockPreparedStatement {
    parameter_meta_data: Arc<dyn ParameterMetaData>,
}

druid_rdbc::impl_standard_wrapper!(MockConnection => Connection);
druid_rdbc::impl_standard_wrapper!(MockDatabaseMetaData => DatabaseMetaData);
druid_rdbc::impl_standard_wrapper!(MockResultSet => ResultSet);
druid_rdbc::impl_standard_wrapper!(MockResultSetMetaData => ResultSetMetaData);
druid_rdbc::impl_standard_wrapper!(MockParameterMetaData => ParameterMetaData);
druid_rdbc::impl_standard_wrapper!(MockPreparedStatement => PreparedStatement);

impl ResultSetMetaData for MockResultSetMetaData {
    fn column_count(&self) -> usize {
        1
    }
}

impl ParameterMetaData for MockParameterMetaData {
    fn parameter_count(&self) -> usize {
        1
    }
}

#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "metadata_fixture_create_statement",
        })
    }

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        Ok(Box::new(MockDatabaseMetaData(self.0.clone())))
    }
}

#[async_trait::async_trait]
impl DatabaseMetaData for MockDatabaseMetaData {
    async fn tables(&mut self, _query: TablesQuery) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(MockResultSet {
            actor: self.0.clone(),
            meta_data: Arc::new(MockResultSetMetaData),
            advanced: false,
        }))
    }
}

#[async_trait::async_trait]
impl ResultSet for MockResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        if self.advanced {
            return Ok(false);
        }
        self.advanced = true;
        self.actor.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(self.meta_data.clone())
    }
}

#[async_trait::async_trait]
impl Statement for MockPreparedStatement {}

#[async_trait::async_trait]
impl PreparedStatement for MockPreparedStatement {
    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Ok(self.parameter_meta_data.clone())
    }
}

fn assert_send_sync_static<T: Send + Sync + 'static>(_: T) {}

#[tokio::test]
async fn live_metadata_and_snapshots_have_distinct_owned_lifetimes() -> Result<(), RdbcError> {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut connection = MockConnection(ActorHandle(counter.clone()));
    let mut database_meta_data = connection.database_meta_data().await?;
    drop(connection);

    let mut tables = database_meta_data.tables(TablesQuery::default()).await?;
    drop(database_meta_data);
    assert!(tables.next().await?);

    let result_set_meta_data = tables.meta_data()?;
    drop(tables);
    assert_eq!(result_set_meta_data.column_count(), 1);
    assert_send_sync_static(result_set_meta_data);

    let prepared = MockPreparedStatement {
        parameter_meta_data: Arc::new(MockParameterMetaData),
    };
    let parameter_meta_data = prepared.parameter_meta_data()?;
    drop(prepared);
    assert_eq!(parameter_meta_data.parameter_count(), 1);
    assert_send_sync_static(parameter_meta_data);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    Ok(())
}
