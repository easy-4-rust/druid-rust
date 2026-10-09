//! 明确停止在 driver await，验证取消不重放已经消费的资源批次。

use super::StandardAdapterPreparedStatement;
use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore};
use crate::standard_query_result::StandardQueryResult;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream, RdbcStreamLength};
use druid_rdbc::statement::{
    ParameterIndex, PreparedInputParameter, PreparedStatement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// 驱动等待可控，但实际使用生产 executor、参数状态与材料化入口。
struct WaitingDriverCore {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl AdapterExecutionCore for WaitingDriverCore {
    async fn standard_execute_update_core(
        &mut self,
        _sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        assert_eq!(values, vec![Value::Bytes(b"first".to_vec())]);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
        Ok(1)
    }
    async fn standard_execute_query_core(
        &mut self,
        _sql: &str,
        _values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "waiting_test_query",
        })
    }
    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancellation_during_driver_await_does_not_replay_stream_batches() {
    let entered = Arc::new(Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let executor = owned_executor(
        WaitingDriverCore {
            entered: Arc::clone(&entered),
            release: Arc::new(Notify::new()),
            calls: Arc::clone(&calls),
        },
        None,
    );
    let mut statement = StandardAdapterPreparedStatement::new(
        "INSERT INTO stream_item(value) VALUES (?)".to_owned(),
        executor,
        Some(1),
        StatementOptions::default(),
    );
    let first = RdbcInputStream::from_bytes(b"first");
    let second = RdbcInputStream::from_bytes(b"second");
    for stream in [&first, &second] {
        statement
            .add_input_parameter_batch(vec![(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(stream.clone()),
                    length: RdbcStreamLength::Unspecified,
                },
            )])
            .unwrap();
    }
    let mut execution = Box::pin(statement.execute_prepared_batch());
    tokio::select! {
        result = &mut execution => panic!("must stop at driver await: {result:?}"),
        () = entered.notified() => {}
    }
    drop(execution);
    assert_eq!(
        statement.execute_prepared_batch().await.unwrap(),
        Vec::<i64>::new()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(first.read_to_end().unwrap().is_empty());
    assert_eq!(second.read_to_end().unwrap(), b"second");
}
