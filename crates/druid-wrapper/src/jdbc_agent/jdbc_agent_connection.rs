use super::jdbc_agent_array::JdbcAgentArray;
use super::jdbc_agent_blob::JdbcAgentBlob;
use super::jdbc_agent_callable_statement::JdbcAgentCallableStatement;
use super::jdbc_agent_cancellation_context::JdbcAgentCancellationContext;
use super::{AgentValue, JdbcAgentClient, JdbcAgentDatabaseMetaData, JdbcAgentOptions};
use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

mod jdbc_agent_support;
pub use jdbc_agent_support::JdbcAgentConnection;
pub(crate) use jdbc_agent_support::{
    JdbcAgentConnector, JdbcAgentTransport, NativeJdbcAgentConnector,
};

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for JdbcAgentConnection {
    async fn create_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Clob>, RdbcError> {
        self.ensure_open()?;
        let executor = self.standard_executor_core()?;
        let response = executor
            .connection_control("create_clob", json!({}))
            .await?;
        let clob_id = response
            .get("clobId")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Self::protocol_error("create_clob returned no clobId"))?;
        Ok(Box::new(super::jdbc_agent_clob::JdbcAgentClob::new(
            executor,
            clob_id.to_owned(),
        )))
    }

    async fn create_n_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::NClob>, RdbcError> {
        self.ensure_open()?;
        let executor = self.standard_executor_core()?;
        let response = executor
            .connection_control("create_n_clob", json!({}))
            .await?;
        let n_clob_id = response
            .get("nClobId")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Self::protocol_error("create_n_clob returned no nClobId"))?;
        Ok(Box::new(super::jdbc_agent_n_clob::JdbcAgentNClob::new(
            executor,
            n_clob_id.to_owned(),
        )))
    }

    async fn create_blob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Blob>, RdbcError> {
        self.ensure_open()?;
        let executor = self.standard_executor_core()?;
        let response = executor
            .connection_control("create_blob", json!({}))
            .await?;
        let blob_id = response
            .get("blobId")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Self::protocol_error("create_blob returned no blobId"))?;
        Ok(Box::new(JdbcAgentBlob::new(executor, blob_id.to_owned())))
    }

    async fn create_array_of(
        &mut self,
        type_name: Option<&str>,
        elements: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Array>, RdbcError> {
        self.ensure_open()?;
        let encoded = elements
            .map(|values| {
                values
                    .into_iter()
                    .map(AgentValue::from_object)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let executor = self.standard_executor_core()?;
        let response = executor
            .connection_control(
                "create_array",
                json!({
                    "typeName": type_name,
                    "elements": encoded,
                }),
            )
            .await?;
        let array_id = response
            .get("arrayId")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Self::protocol_error("create_array returned no arrayId"))?;
        Ok(Box::new(JdbcAgentArray::new(executor, array_id.to_owned())))
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.auto_commit)
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_auto_commit", json!({"value": enabled}))
            .await?;
        self.auto_commit = enabled;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("commit", JsonValue::Null)
            .await?;
        self.savepoints.clear();
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("rollback", JsonValue::Null)
            .await?;
        self.savepoints.clear();
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        self.create_savepoint(None).await
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        self.create_savepoint(Some(name)).await
    }

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let remote_id = self
            .savepoints
            .get(&savepoint.id)
            .ok_or_else(|| RdbcError::InvalidArgument("unknown JDBC Agent savepoint".to_owned()))?
            .clone();
        self.standard_executor_core()?
            .connection_control("rollback_to_savepoint", json!({"savepointId": remote_id}))
            .await?;
        Ok(())
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let remote_id = self
            .savepoints
            .get(&savepoint.id)
            .ok_or_else(|| RdbcError::InvalidArgument("unknown JDBC Agent savepoint".to_owned()))?
            .clone();
        self.standard_executor_core()?
            .connection_control("release_savepoint", json!({"savepointId": remote_id}))
            .await?;
        self.savepoints.remove(&savepoint.id);
        Ok(())
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.catalog.clone())
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_catalog", json!({"value": catalog}))
            .await?;
        self.catalog = catalog.map(str::to_owned);
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.schema.clone())
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_schema", json!({"value": schema}))
            .await?;
        self.schema = schema.map(str::to_owned);
        Ok(())
    }

    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = self
            .standard_executor_core()?
            .execute_update_result(sql, params)
            .await?;
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.standard_executor_core()?
            .execute_query(sql, params)
            .await
            .map(|result| result.rows)
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        let java_timeout = i32::try_from(timeout_seconds).map_err(|_| {
            RdbcError::InvalidArgument("JDBC validation timeout exceeds Java int range".to_owned())
        })?;
        let executor = self.standard_executor_core()?;
        // 由 Agent 优先调用 JDBC Connection#isValid；驱动不支持时才执行配置的校验 SQL。
        let validation = executor.connection_control(
            "validate_connection",
            json!({"timeoutSeconds": java_timeout}),
        );
        if timeout_seconds == 0 {
            validation.await?;
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(u64::from(timeout_seconds)),
                validation,
            )
            .await
            .map_err(|_| {
                RdbcError::DriverError("JDBC Agent connection validation timed out".to_owned())
            })??;
        }
        Ok(true)
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
            options,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
            options,
        )))
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::CallableStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let payload = if options == druid_rdbc::statement::StatementOptions::default() {
            json!({"sql": sql})
        } else {
            json!({
                "sql": sql,
                "resultSetType": options.result_set_type().rdbc_code(),
                "concurrency": options.concurrency().rdbc_code(),
                "holdability": options.holdability().rdbc_code(),
            })
        };
        let response = executor.connection_control("prepare_call", payload).await?;
        let statement_id = response
            .get("statementId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Self::protocol_error("prepare_call returned no statementId"))?;
        Ok(Box::new(JdbcAgentCallableStatement::new(
            sql.to_owned(),
            statement_id.to_owned(),
            options,
            executor,
            self.cancellation
                .as_ref()
                .map(|context| (Arc::clone(context), context.register(statement_id))),
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let mut snapshot = self.metadata.clone();
        let product_name =
            druid_rdbc::metadata::DatabaseMetaData::database_product_name(&mut snapshot)
                .await
                .ok();
        Ok(Box::new(
            StandardAdapterDatabaseMetaData::new(
                self.standard_executor_core()?,
                product_name,
                "jdbc_agent_database_metadata_product_name_absent",
            )
            .with_driver_name(self.driver_name.clone()),
        ))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_executor.as_ref().map_or_else(
            || {
                self.closed
                    || self.discarded
                    || self
                        .client
                        .as_ref()
                        .is_none_or(|client| client.is_unusable())
            },
            |executor| executor.is_closed(),
        ))
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        self.capabilities
    }
}

#[cfg(test)]
mod callable_tests {
    use super::{JdbcAgentConnection, JdbcAgentTransport};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::result_set::{ColumnIndex, ResultSetExt, ResultSetNavigation};
    use druid_rdbc::statement::{
        CallableOutParameter, CallableParameter, ExecutionResult, ParameterIndex,
        PreparedParameter, StatementOptions,
    };
    use druid_rdbc::types::Value;
    use serde_json::{json, Value as JsonValue};
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    struct CallableTransport(Arc<Mutex<Vec<(String, JsonValue)>>>);

    struct PagedCallableTransport(Arc<Mutex<Vec<String>>>);

    struct CancelledPageTransport {
        calls: Arc<Mutex<Vec<String>>>,
        fetching: Arc<Notify>,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for CancelledPageTransport {
        async fn request(
            &mut self,
            operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.calls.lock().unwrap().push(operation.to_owned());
            if operation == "fetch_page" {
                self.fetching.notify_one();
                return std::future::pending().await;
            }
            Ok(match operation {
                "prepare_call" => json!({"statementId":"call-cancel"}),
                "execute_callable" => json!({
                    "kind":"result_set", "cursorId":"cursor-cancel",
                    "columns":[{"label":"value"}],
                    "rows":[[{"kind":"int","value":1}]], "hasMore":true,
                    "outValues":[]
                }),
                "close_cursor" => json!({}),
                _ => panic!("unexpected operation {operation}"),
            })
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn cancelled_callable_page_fetch_closes_remote_cursor() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let fetching = Arc::new(Notify::new());
        let mut connection = JdbcAgentConnection::from_session(
            Box::new(CancelledPageTransport {
                calls: Arc::clone(&calls),
                fetching: Arc::clone(&fetching),
            }),
            json!({"capabilities":{"callableStatements":true}}),
        );
        let mut callable = connection
            .prepare_call("CALL cancelled()", StatementOptions::default())
            .await
            .unwrap();
        let running = tokio::spawn(async move { callable.execute_prepared().await });
        tokio::time::timeout(std::time::Duration::from_secs(5), fetching.notified())
            .await
            .expect("fetch must start");
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if calls
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|call| call == "close_cursor")
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled fetch must close its cursor");
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for PagedCallableTransport {
        async fn request(
            &mut self,
            operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.0
                .lock()
                .expect("request lock")
                .push(operation.to_owned());
            Ok(match operation {
                "prepare_call" => json!({"statementId":"call-paged"}),
                "execute_callable" => json!({
                    "kind":"result_set", "cursorId":"cursor-paged",
                    "columns":[{"label":"value"}],
                    "rows":(1..=500).map(|value| vec![json!({"kind":"int","value":value})]).collect::<Vec<_>>(),
                    "hasMore":true, "outValues":[]
                }),
                "fetch_page" => json!({
                    "kind":"result_set", "cursorId":"cursor-paged",
                    "rows":[[{"kind":"int","value":501}]], "hasMore":false
                }),
                "next_callable_result" => json!({"kind":"done"}),
                "close_statement" => json!({}),
                _ => panic!("unexpected operation {operation}"),
            })
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn callable_spools_multiple_agent_pages_without_losing_navigation() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut connection = JdbcAgentConnection::from_session(
            Box::new(PagedCallableTransport(Arc::clone(&calls))),
            json!({"capabilities":{"callableStatements":true}}),
        );
        let mut callable = connection
            .prepare_call("CALL paged()", StatementOptions::default())
            .await
            .expect("prepare callable");
        let mut results = callable
            .execute_prepared()
            .await
            .expect("execute paged call");
        let Some(ExecutionResult::ResultSet(mut rows)) = results.pop() else {
            panic!("expected one result set");
        };
        assert_eq!(rows.known_row_count(), Some(501));
        assert!(rows.navigate(ResultSetNavigation::Last).await.unwrap());
        assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 501);
        assert!(rows.navigate(ResultSetNavigation::First).await.unwrap());
        assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
        rows.close().await.unwrap();
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [
                "prepare_call",
                "execute_callable",
                "fetch_page",
                "next_callable_result"
            ]
        );
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for CallableTransport {
        async fn request(
            &mut self,
            operation: &str,
            payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.0
                .lock()
                .expect("request lock")
                .push((operation.to_owned(), payload));
            Ok(match operation {
                "prepare_call" => json!({"statementId":"call-1"}),
                "execute_callable" => json!({"kind":"update","rowsAffected":0,
                    "outValues":[{"index":1,"value":{"kind":"integer","value":7},"wasNull":false}]}),
                "next_callable_result" => json!({"kind":"done"}),
                "close_statement" => json!({}),
                _ => panic!("unexpected operation {operation}"),
            })
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn callable_uses_one_owned_agent_session_for_prepare_execute_and_close() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut connection = JdbcAgentConnection::from_session(
            Box::new(CallableTransport(Arc::clone(&calls))),
            json!({"capabilities":{"callableStatements":true}}),
        );
        assert!(connection.capabilities().callable_statements);
        let mut callable = connection
            .prepare_call("{? = call ABS(?)}", StatementOptions::default())
            .await
            .expect("supported callable preparation");
        callable
            .register_out_parameter(
                CallableParameter::by_index(1).expect("OUT index"),
                CallableOutParameter::new(4),
            )
            .expect("register OUT");
        callable
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(2).expect("IN index"),
                value: Value::Int(-7),
            })
            .expect("bind IN");
        callable.execute_prepared().await.expect("execute call");
        assert_eq!(
            callable
                .out_value(ParameterIndex::new(1).expect("OUT index"))
                .expect("OUT value"),
            Value::Int(7)
        );
        assert!(!callable.was_null().expect("OUT null status"));
        callable.close().await.expect("close callable");
        assert!(callable.out_value(ParameterIndex::new(1).unwrap()).is_err());
        let calls = calls.lock().expect("request lock");
        assert_eq!(
            calls
                .iter()
                .map(|(method, _)| method.as_str())
                .collect::<Vec<_>>(),
            [
                "prepare_call",
                "execute_callable",
                "next_callable_result",
                "close_statement"
            ]
        );
        assert_eq!(calls[1].1["statementId"], "call-1");
        assert_eq!(calls[1].1["inParameters"][0]["index"], 2);
        assert_eq!(calls[1].1["outParameters"][0]["sqlType"], 4);
    }
}

#[cfg(test)]
mod array_tests {
    use super::{JdbcAgentConnection, JdbcAgentTransport};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::resource::ResourceOperations;
    use druid_rdbc::statement::StatementOptions;
    use druid_rdbc::types::{RdbcObject, RdbcTypeMap, Value};
    use serde_json::{json, Value as JsonValue};
    use std::sync::{Arc, Mutex};

    struct ArrayTransport {
        requests: Arc<Mutex<Vec<(String, JsonValue)>>>,
        fail_first_free: bool,
        free_attempts: usize,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for ArrayTransport {
        async fn request(
            &mut self,
            operation: &str,
            payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            if operation == "free_array" {
                self.free_attempts += 1;
                if self.fail_first_free && self.free_attempts == 1 {
                    return Err(RdbcError::DriverError(
                        "injected Array free failure".to_owned(),
                    ));
                }
            }
            self.requests
                .lock()
                .expect("requests lock")
                .push((operation.to_owned(), payload));
            Ok(match operation {
                "create_array" => json!({"arrayId":"array-1"}),
                "array_metadata" => json!({"baseTypeName":"INTEGER","baseType":4}),
                "array_values" => json!({"values":[{"kind":"integer","value":42},{"kind":"null"}]}),
                "free_array" => json!({}),
                _ => panic!("unexpected JDBC Agent operation {operation}"),
            })
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn array_factory_uses_owned_executor_after_statement_creation() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = ArrayTransport {
            requests: Arc::clone(&requests),
            fail_first_free: false,
            free_attempts: 0,
        };
        let mut connection =
            JdbcAgentConnection::from_session(Box::new(transport), json!({"capabilities": {}}));
        let _statement = connection
            .create_statement(StatementOptions::default())
            .await
            .expect("statement");
        let mut array = connection
            .create_array_of(
                Some("INTEGER"),
                Some(vec![
                    RdbcObject::Integer(42),
                    RdbcObject::Scalar(Value::Null),
                ]),
            )
            .await
            .expect("array");
        assert_eq!(array.base_type_name().await.expect("name"), "INTEGER");
        assert_eq!(array.base_type().await.expect("type"), 4);
        assert_eq!(
            array.values_range(1, 2).await.expect("values"),
            vec![RdbcObject::Integer(42), RdbcObject::Scalar(Value::Null),]
        );
        assert_eq!(
            array.capabilities(),
            ResourceOperations::READ
                | ResourceOperations::RANGE
                | ResourceOperations::RESULT_SET
                | ResourceOperations::FREE
        );
        let count_before_unsupported = requests.lock().expect("requests lock").len();
        let unsupported = array
            .values_with_type_map(&RdbcTypeMap::new())
            .await
            .expect_err("type-map overload must be rejected before driver call");
        assert_eq!(
            unsupported
                .sql_exception()
                .and_then(|error| error.sql_state()),
            Some("0A000")
        );
        assert_eq!(
            requests.lock().expect("requests lock").len(),
            count_before_unsupported
        );
        array.free().await.expect("free");
        array.free().await.expect("idempotent free");
        assert!(array.values().await.is_err());
        assert!(matches!(
            array.values_with_type_map(&RdbcTypeMap::new()).await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        let recorded = requests.lock().expect("requests lock");
        assert_eq!(recorded[0].0, "create_array");
        assert_eq!(recorded[0].1["elements"][0]["kind"], "integer");
        assert_eq!(recorded[0].1["elements"][1]["kind"], "null");
        assert_eq!(
            recorded
                .iter()
                .filter(|(operation, _)| operation == "free_array")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn array_free_failure_remains_retryable_and_null_factory_arguments_survive() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = ArrayTransport {
            requests: Arc::clone(&requests),
            fail_first_free: true,
            free_attempts: 0,
        };
        let mut connection =
            JdbcAgentConnection::from_session(Box::new(transport), json!({"capabilities": {}}));
        let mut array = connection
            .create_array_of(None, None)
            .await
            .expect("null arguments forwarded");
        assert!(array.free().await.is_err());
        assert_eq!(
            array
                .values()
                .await
                .expect("failed free preserves handle")
                .len(),
            2
        );
        array.free().await.expect("retry free");
        let recorded = requests.lock().expect("requests lock");
        assert!(recorded[0].1["typeName"].is_null());
        assert!(recorded[0].1["elements"].is_null());
    }
}
