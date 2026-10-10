//! 固定 SQLInput/SQLData 错误传播及 Rust Future 取消的所有权合同。
use druid_rdbc::error::{RdbcError, SqlException, SqlExceptionCause};
use druid_rdbc::extensions::{BufferedSqlInput, BufferedSqlOutput, SqlDataRegistry};
use druid_rdbc::resource::{SqlData, SqlInput, SqlOutput};
use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap};
use std::future::{pending, poll_fn, Future};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

fn original_error() -> SqlException {
    let mut error = SqlException::new(913, Some("22005".into()), None)
        .with_class_name("example.MappingFailure")
        .with_cause(SqlExceptionCause::SocketTimeout)
        .with_cause(SqlExceptionCause::ClassName("example.RootCause".into()));
    error.set_next_exception(SqlException::new(914, None, Some("next error".into())));
    error
}

struct Encoded {
    name: String,
    fields: Vec<RdbcObject>,
}

#[async_trait::async_trait]
impl SqlData for Encoded {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok(self.name.clone())
    }
    async fn read_sql(&mut self, input: &mut dyn SqlInput, name: &str) -> Result<(), RdbcError> {
        self.name = name.into();
        self.fields = vec![input.read_object().await?];
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        for field in &self.fields {
            match field {
                RdbcObject::Integer(number) => output.write_int(*number).await?,
                RdbcObject::Struct(value) => output.write_struct(Some(value.clone())).await?,
                other => panic!("unexpected fixture field: {other:?}"),
            }
        }
        Ok(())
    }
}

async fn structure(name: &str, fields: Vec<RdbcObject>) -> RdbcObject {
    let mut encoded = Encoded {
        name: name.into(),
        fields,
    };
    let mut output = BufferedSqlOutput::new();
    output.write_object(Some(&mut encoded)).await.unwrap();
    output.into_input().read_object().await.unwrap()
}

#[derive(Clone, Copy)]
enum ReadBehavior {
    Fail,
    Wait,
    Nested,
}

struct TracedData {
    dropped: Arc<AtomicUsize>,
    started: Arc<AtomicUsize>,
    behavior: ReadBehavior,
}

impl Drop for TracedData {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl SqlData for TracedData {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("APP.RECORD".into())
    }
    async fn read_sql(&mut self, input: &mut dyn SqlInput, _: &str) -> Result<(), RdbcError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        match self.behavior {
            ReadBehavior::Fail => {
                assert_eq!(input.read_int().await?, 71);
                Err(RdbcError::SqlException(Box::new(original_error())))
            }
            ReadBehavior::Wait => {
                assert_eq!(input.read_int().await?, 71);
                pending().await
            }
            ReadBehavior::Nested => {
                input.read_object().await?;
                Ok(())
            }
        }
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(71).await
    }
}

fn register(
    registry: &mut SqlDataRegistry,
    class: &str,
    behavior: ReadBehavior,
    dropped: &Arc<AtomicUsize>,
    started: &Arc<AtomicUsize>,
) {
    let dropped = Arc::clone(dropped);
    let started = Arc::clone(started);
    registry.register(class, move || {
        Ok(Box::new(TracedData {
            dropped: Arc::clone(&dropped),
            started: Arc::clone(&started),
            behavior,
        }) as Box<dyn SqlData>)
    });
}

async fn input(
    behavior: ReadBehavior,
    nested: bool,
    dropped: &Arc<AtomicUsize>,
    started: &Arc<AtomicUsize>,
) -> BufferedSqlInput {
    let mut registry = SqlDataRegistry::new();
    register(&mut registry, "example.Record", behavior, dropped, started);
    let mut map = RdbcTypeMap::new();
    map.insert(
        "APP.RECORD",
        RdbcTargetType::Custom("example.Record".into()),
    );
    let mut value = structure("APP.RECORD", vec![RdbcObject::Integer(71)]).await;
    if nested {
        register(
            &mut registry,
            "example.Parent",
            ReadBehavior::Nested,
            dropped,
            started,
        );
        map.insert(
            "APP.PARENT",
            RdbcTargetType::Custom("example.Parent".into()),
        );
        value = structure("APP.PARENT", vec![value]).await;
    }
    BufferedSqlInput::with_type_map(
        vec![value, RdbcObject::Integer(99)],
        map,
        Arc::new(registry),
    )
}

async fn assert_read_failure(nested: bool) {
    let dropped = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicUsize::new(0));
    let mut input = input(ReadBehavior::Fail, nested, &dropped, &started).await;
    let Err(RdbcError::SqlException(error)) = input.read_object().await else {
        panic!("readSQL must preserve the original SQL exception rather than publish Custom")
    };
    assert_eq!(
        *error,
        original_error(),
        "all SQLState/vendor/class/cause/next/null fields must survive"
    );
    let expected = if nested { 2 } else { 1 };
    assert_eq!(started.load(Ordering::SeqCst), expected);
    assert_eq!(dropped.load(Ordering::SeqCst), expected);
    assert!(!input.was_null().unwrap());
    assert_eq!(
        input.read_int().await.unwrap(),
        99,
        "failed conversion consumes only its original attribute"
    );
    drop(input);
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        expected,
        "no duplicate destruction"
    );
}

async fn assert_read_cancellation(nested: bool) {
    let dropped = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicUsize::new(0));
    let mut input = input(ReadBehavior::Wait, nested, &dropped, &started).await;
    let expected = if nested { 2 } else { 1 };
    {
        let future = input.read_object();
        tokio::pin!(future);
        poll_fn(|context| {
            assert!(future.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            expected,
            "cancel only after actual readSQL entry"
        );
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            0,
            "owned partial instances are live while pending"
        );
    }
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        expected,
        "cancellation drops every unpublished instance once"
    );
    assert_eq!(
        input.read_int().await.unwrap(),
        99,
        "cancellation cannot consume the following attribute"
    );
    drop(input);
    assert_eq!(dropped.load(Ordering::SeqCst), expected);
}

#[tokio::test]
async fn read_sql_error_preserves_complete_exception_and_destroys_partial_instance() {
    assert_read_failure(false).await;
}

#[tokio::test]
async fn nested_read_sql_error_preserves_exception_and_destroys_both_partial_instances() {
    assert_read_failure(true).await;
}

#[tokio::test]
async fn cancellation_after_read_sql_entry_drops_unpublished_instance_and_preserves_next_attribute()
{
    assert_read_cancellation(false).await;
}

#[tokio::test]
async fn nested_cancellation_drops_both_unpublished_instances_and_preserves_next_attribute() {
    assert_read_cancellation(true).await;
}
