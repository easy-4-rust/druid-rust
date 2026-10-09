mod row_operation_filter_support;
use druid::core::{FilterChain, RdbcError, ResultSetRowOperation};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::StatementOptions;
use druid_wrapper::toasty::ToastyConnectionFactory;
use row_operation_filter_support::OrderedRowOperationFilter;
use std::sync::{Arc, Mutex};

fn invoke(rows: &mut dyn ResultSet, operation: ResultSetRowOperation) -> Result<(), RdbcError> {
    match operation {
        ResultSetRowOperation::InsertRow => rows.insert_row(),
        ResultSetRowOperation::UpdateRow => rows.update_row(),
        ResultSetRowOperation::DeleteRow => rows.delete_row(),
        ResultSetRowOperation::RefreshRow => rows.refresh_row(),
        ResultSetRowOperation::CancelRowUpdates => rows.cancel_row_updates(),
        ResultSetRowOperation::MoveToInsertRow => rows.move_to_insert_row(),
        ResultSetRowOperation::MoveToCurrentRow => rows.move_to_current_row(),
    }
}

/// 外部公开Filter通过真实SQLite池验证每个行操作的顺序、能力拒绝和门禁。
#[tokio::test]
async fn external_row_operation_filters_preserve_order_short_circuit_errors_and_guards() {
    let operations = [
        (ResultSetRowOperation::InsertRow, "result_set_insert_row"),
        (ResultSetRowOperation::UpdateRow, "result_set_update_row"),
        (ResultSetRowOperation::DeleteRow, "result_set_delete_row"),
        (ResultSetRowOperation::RefreshRow, "result_set_refresh_row"),
        (
            ResultSetRowOperation::CancelRowUpdates,
            "result_set_cancel_row_updates",
        ),
        (
            ResultSetRowOperation::MoveToInsertRow,
            "result_set_move_to_insert_row",
        ),
        (
            ResultSetRowOperation::MoveToCurrentRow,
            "result_set_move_to_current_row",
        ),
    ];
    for mode in [0, 1, 2] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedRowOperationFilter {
                name,
                mode: if name == "B" { mode } else { 0 },
                calls: Arc::clone(&calls),
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
        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
        for (operation, native_name) in operations {
            calls.lock().unwrap().clear();
            let result = invoke(rows.as_mut(), operation);
            if mode == 0 {
                assert_eq!(
                    result,
                    Err(RdbcError::UnsupportedOperation {
                        operation: native_name
                    })
                );
            } else if mode == 1 {
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(
                    result,
                    Err(RdbcError::DriverError(
                        "ordered row operation failure".to_owned()
                    ))
                );
            }
            let status = if mode == 1 { "ok" } else { "error" };
            let order = if mode == 0 {
                vec![
                    ("A", "before"),
                    ("B", "before"),
                    ("C", "before"),
                    ("C", status),
                    ("B", status),
                    ("A", status),
                ]
            } else {
                vec![
                    ("A", "before"),
                    ("B", "before"),
                    ("B", status),
                    ("A", status),
                ]
            };
            assert_eq!(
                *calls.lock().unwrap(),
                order
                    .into_iter()
                    .map(|(name, status)| format!("{name}:{operation:?}:{status}"))
                    .collect::<Vec<_>>()
            );
        }
        // 短路或错误都不应修改真实SQLite结果或推进游标。
        assert!(rows.next().await.unwrap());
        assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
        rows.close().await.unwrap();
        calls.lock().unwrap().clear();
        for (operation, _) in operations {
            assert_eq!(
                invoke(rows.as_mut(), operation),
                Err(RdbcError::ConnectionDiscarded)
            );
        }
        assert!(calls.lock().unwrap().is_empty());
        let mut expired = statement.execute_query("SELECT 2 AS value").await.unwrap();
        connection.close().await.unwrap();
        calls.lock().unwrap().clear();
        let mut next_lease = pool.get().await.unwrap();
        for (operation, _) in operations {
            assert_eq!(
                invoke(expired.as_mut(), operation),
                Err(RdbcError::ConnectionDiscarded)
            );
        }
        assert!(calls.lock().unwrap().is_empty());
        next_lease.close().await.unwrap();
        pool.close().await;
    }
}
