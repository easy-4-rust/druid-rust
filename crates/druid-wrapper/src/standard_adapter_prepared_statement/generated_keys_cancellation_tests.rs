//! 在批次第二项驱动等待时取消，验证真实 prepared/executor 的结果发布边界。

use super::StandardAdapterPreparedStatement;
use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore, AdapterUpdateResult};
use crate::standard_query_result::StandardQueryResult;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    ParameterIndex, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// 只控制驱动完成时机；参数材料化、执行器锁和结果状态均使用生产实现。
struct BatchWaitingDriverCore {
    calls: Arc<AtomicUsize>,
    entered: Arc<Notify>,
}

#[async_trait::async_trait]
impl AdapterExecutionCore for BatchWaitingDriverCore {
    async fn standard_execute_update_core(
        &mut self,
        _sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(values, vec![Value::Int(call as i64)]);
        if call == 3 {
            self.entered.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(1)
    }

    async fn standard_execute_update_result_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        self.standard_execute_update_core(sql, values).await?;
        Ok(AdapterUpdateResult {
            rows_affected: 1,
            last_insert_id: Some(self.calls.load(Ordering::SeqCst) as i64),
            row_count: None,
        })
    }

    async fn standard_execute_query_core(
        &mut self,
        _sql: &str,
        _values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "batch_waiting_query",
        })
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[tokio::test]
async fn cancelled_second_batch_item_does_not_publish_partial_or_previous_keys() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let executor = owned_executor(
        BatchWaitingDriverCore {
            calls: Arc::clone(&calls),
            entered: Arc::clone(&entered),
        },
        None,
    );
    let mut prepared = StandardAdapterPreparedStatement::new(
        "INSERT INTO key_item(value) VALUES (?)".into(),
        executor,
        Some(1),
        StatementOptions::default(),
    );
    for value in [1, 2, 3] {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(value),
            })
            .unwrap();
        if value == 1 {
            // 保留未消费的上一轮成功键，下一次尝试必须先使其失效。
            assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        } else {
            prepared.add_prepared_batch().unwrap();
        }
    }
    let mut execution = Box::pin(prepared.execute_prepared_batch());
    tokio::select! {
        result = &mut execution => panic!("应停在第二项驱动等待：{result:?}"),
        () = entered.notified() => {}
    }
    drop(execution);
    assert!(prepared.get_update_count().unwrap().is_none());
    let mut keys = prepared.generated_keys().await.unwrap();
    assert!(
        !keys.next().await.unwrap(),
        "取消不得发布上一轮或本批部分键"
    );
    keys.close().await.unwrap();
    assert_eq!(
        prepared.execute_prepared_batch().await.unwrap(),
        Vec::<i64>::new()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3, "取消后的批次不得重放");
    prepared.close().await.unwrap();
}
