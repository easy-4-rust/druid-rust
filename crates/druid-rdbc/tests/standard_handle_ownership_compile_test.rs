use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::wrapper::Wrapper;
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct ActorHandle(Arc<AtomicUsize>);

struct MockConnection {
    actor: ActorHandle,
}

struct MockStatement {
    actor: ActorHandle,
}

struct MockResultSet {
    actor: ActorHandle,
    advanced: bool,
}

macro_rules! impl_wrapper {
    ($($target:ty),+ $(,)?) => {
        $(
            impl Wrapper for $target {
                fn as_any(&self) -> &dyn Any {
                    self
                }
            }
        )+
    };
}

impl_wrapper!(MockConnection, MockStatement, MockResultSet);

#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(MockStatement {
            actor: self.actor.clone(),
        }))
    }
}

#[async_trait::async_trait]
impl Statement for MockStatement {
    async fn execute_query(&mut self, _sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(Box::new(MockResultSet {
            actor: self.actor.clone(),
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
}

fn assert_send_static<T: Send + 'static>(_: T) {}

#[tokio::test]
async fn statement_and_result_set_outlive_their_creator_calls() -> Result<(), RdbcError> {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut connection = MockConnection {
        actor: ActorHandle(counter.clone()),
    };

    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await?;
    drop(connection);

    let mut result_set = statement.execute_query("select 1").await?;
    drop(statement);

    assert!(result_set.next().await?);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_send_static(result_set);
    Ok(())
}
