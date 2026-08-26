//! Task 5：真实 standard adapter 进入 Druid native pool 的生产边界测试。

use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::StatementOptions;
#[cfg(feature = "duckdb-native")]
use druid_wrapper::duckdb::DuckDbConnectionFactory;
use druid_wrapper::rbdc::RbdcConnectionFactory;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use futures::future::BoxFuture;
use futures::stream::{self, BoxStream, StreamExt};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn sqlx_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool() {
    let factory: Arc<dyn ConnectionFactory> =
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:"));
    exercise_standard_pool("sqlx", factory).await;
}

#[tokio::test]
async fn toasty_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool() {
    let factory: Arc<dyn ConnectionFactory> = Arc::new(
        ToastyConnectionFactory::new("sqlite::memory:")
            .await
            .expect("configured Toasty factory"),
    );
    exercise_standard_pool("toasty", factory).await;
}

#[tokio::test]
async fn rbdc_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool() {
    let factory: Arc<dyn ConnectionFactory> = Arc::new(RbdcConnectionFactory::new(
        Arc::new(AdmissionRbdcDriver),
        "rbdc-admission://local",
    ));
    exercise_standard_pool("rbdc", factory).await;
}

#[tokio::test]
#[cfg(feature = "duckdb-native")]
async fn duckdb_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool() {
    let factory: Arc<dyn ConnectionFactory> =
        Arc::new(DuckDbConnectionFactory::new("duckdb::memory:"));
    exercise_standard_pool("duckdb", factory).await;
}

async fn exercise_standard_pool(name: &str, factory: Arc<dyn ConnectionFactory>) {
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        acquire_timeout: Duration::from_secs(2),
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        format!("task5-{name}-admission"),
        name.to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("standard adapter pool init");
    let mut connection = pool
        .get()
        .await
        .expect("standard factory must enter a Druid holder");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("canonical Druid statement");
    let mut result = statement
        .execute_query("SELECT 1")
        .await
        .expect("canonical query through production adapter");
    assert!(ResultSet::next(result.as_mut()).await.expect("one row"));

    result.close().await.expect("result close");
    statement.close().await.expect("statement close");
    connection.close().await.expect("logical close");
    pool.close().await;
}

#[derive(Debug)]
struct AdmissionRbdcMetaData;

impl rbdc::db::MetaData for AdmissionRbdcMetaData {
    fn column_len(&self) -> usize {
        1
    }

    fn column_name(&self, _index: usize) -> String {
        "value".to_owned()
    }

    fn column_type(&self, _index: usize) -> String {
        "BIGINT".to_owned()
    }
}

#[derive(Debug)]
struct AdmissionRbdcRow;

impl rbdc::db::Row for AdmissionRbdcRow {
    fn meta_data(&self) -> Box<dyn rbdc::db::MetaData> {
        Box::new(AdmissionRbdcMetaData)
    }

    fn get(&mut self, index: usize) -> Result<rbs::Value, rbdc::Error> {
        if index == 0 {
            Ok(rbs::Value::I64(1))
        } else {
            Err(rbdc::Error::from("column index out of bounds"))
        }
    }
}

#[derive(Debug, Default)]
struct AdmissionRbdcConnection;

impl rbdc::db::Connection for AdmissionRbdcConnection {
    fn exec_rows(
        &mut self,
        _sql: &str,
        _params: Vec<rbs::Value>,
    ) -> BoxFuture<
        '_,
        Result<BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>>, rbdc::Error>,
    > {
        Box::pin(async {
            let rows: BoxStream<'_, Result<Box<dyn rbdc::db::Row>, rbdc::Error>> = stream::iter(
                vec![Ok(Box::new(AdmissionRbdcRow) as Box<dyn rbdc::db::Row>)],
            )
            .boxed();
            Ok(rows)
        })
    }

    fn exec(
        &mut self,
        _sql: &str,
        _params: Vec<rbs::Value>,
    ) -> BoxFuture<'_, Result<rbdc::db::ExecResult, rbdc::Error>> {
        Box::pin(async {
            Ok(rbdc::db::ExecResult {
                rows_affected: 1,
                last_insert_id: rbs::Value::Null,
            })
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), rbdc::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn close(&mut self) -> BoxFuture<'_, Result<(), rbdc::Error>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Debug)]
struct AdmissionRbdcConnectOptions;

impl rbdc::db::ConnectOptions for AdmissionRbdcConnectOptions {
    fn connect(&self) -> BoxFuture<'_, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        Box::pin(async { Ok(Box::new(AdmissionRbdcConnection) as Box<dyn rbdc::db::Connection>) })
    }

    fn set_uri(&mut self, _uri: &str) -> Result<(), rbdc::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct AdmissionRbdcDriver;

impl rbdc::db::Driver for AdmissionRbdcDriver {
    fn name(&self) -> &'static str {
        "task5-rbdc-admission"
    }

    fn connect(
        &self,
        _url: &str,
    ) -> BoxFuture<'_, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        Box::pin(async { Ok(Box::new(AdmissionRbdcConnection) as Box<dyn rbdc::db::Connection>) })
    }

    fn connect_opt<'a>(
        &'a self,
        options: &'a dyn rbdc::db::ConnectOptions,
    ) -> BoxFuture<'a, Result<Box<dyn rbdc::db::Connection>, rbdc::Error>> {
        options.connect()
    }

    fn default_option(&self) -> Box<dyn rbdc::db::ConnectOptions> {
        Box::new(AdmissionRbdcConnectOptions)
    }
}
