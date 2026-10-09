//! Canonical parameterized replacement for the removed ResultSet overload matrix.
#![allow(deprecated)]

use druid::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::types::{
    RdbcCharacterLength, RdbcInputStream, RdbcObject, RdbcReader, RdbcStreamLength, Value,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn task6_result_set_filter_case_ids() -> &'static [&'static str] {
    &[
        "result_set_n_string_update_filter_defaults_preserve_both_overloads",
        "result_set_object_update_filter_defaults_preserve_all_four_overloads",
    ]
}

#[derive(Default)]
struct ResultSetProbe {
    next_before: AtomicUsize,
    next_after: AtomicUsize,
    value_before: AtomicUsize,
    value_after: AtomicUsize,
    metadata_before: AtomicUsize,
    metadata_after: AtomicUsize,
    close_before: AtomicUsize,
    close_after: AtomicUsize,
}

#[async_trait::async_trait]
impl ResultSetFilter for ResultSetProbe {
    fn standard_result_set_next_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.next_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_next_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<bool, DruidError>,
    ) -> Result<(), DruidError> {
        self.next_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
    ) -> Result<(), DruidError> {
        self.value_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
        _result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        self.value_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_meta_data_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.metadata_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_meta_data_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &mut Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<(), DruidError> {
        self.metadata_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_close_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.close_before.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn standard_result_set_close_after(
        &self,
        _context: &ResultSetFilterContext,
        _result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        self.close_after.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

async fn task6_assert_result_set_updates_are_unsupported(
    connection: &mut druid::core::DruidPooledConnection,
    updates: Vec<ResultSetUpdate>,
) {
    let mut statement = Connection::create_statement(connection, StatementOptions::default())
        .await
        .unwrap();
    let mut result_set = Statement::execute_query(
        &mut *statement,
        "SELECT 7 AS id, 'value' AS label, NULL AS absent",
    )
    .await
    .unwrap();
    assert!(ResultSet::next(&mut *result_set).await.unwrap());
    let label = ColumnIndex::new(2).unwrap();
    for update in updates {
        assert_eq!(
            ResultSet::update_value(&mut *result_set, label, update.clone()),
            Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
                operation: "result_set_update_value"
            })
        );
        assert_eq!(
            ResultSet::update_value_by_label(&mut *result_set, "label", update),
            Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
                operation: "result_set_update_value_by_label"
            })
        );
    }
    ResultSet::close(&mut *result_set).await.unwrap();
    assert!(ResultSet::is_closed(&*result_set).unwrap());
    Statement::close(&mut *statement).await.unwrap();
}

#[tokio::test]
async fn task6_result_set_filter_canonical_matrix() {
    let probe = Arc::new(ResultSetProbe::default());
    let mut chain = FilterChain::new();
    chain.add_result_set(probe.clone());
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory");
    let pool = DruidPool::builder()
        .name("task6-result-set-filter-matrix")
        .driver_name("toasty")
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(chain))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("canonical pool");
    let mut connection = pool.get().await.expect("canonical lease");
    let cases = task6_result_set_filter_case_ids();
    let mut executed = Vec::with_capacity(cases.len());

    for case_id in cases {
        match *case_id {
            "pooled_result_set_routes_n_string_updates_and_preserves_sqlite_capability_errors"
            | "result_set_n_string_update_filter_defaults_preserve_both_overloads" => {
                task6_assert_result_set_updates_are_unsupported(
                    &mut connection,
                    vec![ResultSetUpdate::NString(Some("updated".to_owned()))],
                )
                .await;
                executed.push(*case_id);
                continue;
            }
            "pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors"
            | "result_set_object_update_filter_defaults_preserve_all_four_overloads" => {
                task6_assert_result_set_updates_are_unsupported(
                    &mut connection,
                    vec![
                        ResultSetUpdate::Object(RdbcObject::Integer(9)),
                        ResultSetUpdate::ObjectWithScaleOrLength {
                            value: RdbcObject::String("object".to_owned()),
                            scale_or_length: 2,
                        },
                    ],
                )
                .await;
                executed.push(*case_id);
                continue;
            }
            "pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors" => {
                task6_assert_result_set_updates_are_unsupported(
                    &mut connection,
                    vec![
                        ResultSetUpdate::Null,
                        ResultSetUpdate::Boolean(true),
                        ResultSetUpdate::Byte(1),
                        ResultSetUpdate::Short(2),
                        ResultSetUpdate::Int(3),
                        ResultSetUpdate::Long(4),
                        ResultSetUpdate::Float(5.0),
                        ResultSetUpdate::Double(6.0),
                        ResultSetUpdate::String(Some("scalar".to_owned())),
                        ResultSetUpdate::Bytes(Some(vec![1, 2, 3])),
                    ],
                )
                .await;
                executed.push(*case_id);
                continue;
            }
            "pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors"
            | "result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads"
            | "result_set_lob_stream_update_filter_defaults_preserve_all_twelve_overloads" => {
                task6_assert_result_set_updates_are_unsupported(
                    &mut connection,
                    vec![
                        ResultSetUpdate::AsciiStream {
                            stream: Some(RdbcInputStream::from_bytes(vec![1, 2])),
                            length: RdbcStreamLength::Long(2),
                        },
                        ResultSetUpdate::BinaryStream {
                            stream: Some(RdbcInputStream::from_bytes(vec![3, 4])),
                            length: RdbcStreamLength::Int(2),
                        },
                        ResultSetUpdate::CharacterStream {
                            reader: Some(RdbcReader::from_string("chars")),
                            length: RdbcCharacterLength::Long(5),
                        },
                        ResultSetUpdate::NCharacterStream {
                            reader: Some(RdbcReader::from_string("national")),
                            length: RdbcCharacterLength::Unspecified,
                        },
                    ],
                )
                .await;
                executed.push(*case_id);
                continue;
            }
            _ => {}
        }
        let mut statement =
            Connection::create_statement(&mut connection, StatementOptions::default())
                .await
                .expect("canonical statement");
        let mut result_set = statement
            .execute_query("SELECT 7 AS id, 'value' AS label, NULL AS absent")
            .await
            .expect("canonical result set");
        let next_before = probe.next_before.load(Ordering::Acquire);
        let value_before = probe.value_before.load(Ordering::Acquire);
        assert!(result_set.next().await.expect("next"), "case {case_id}");
        assert_eq!(probe.next_before.load(Ordering::Acquire), next_before + 1);
        assert_eq!(probe.next_after.load(Ordering::Acquire), next_before + 1);

        let id = ColumnIndex::new(1).expect("column 1");
        let label = ColumnIndex::new(2).expect("column 2");
        let absent = ColumnIndex::new(3).expect("column 3");
        assert_eq!(result_set.value(id).expect("id"), Value::Int(7));
        assert_eq!(probe.value_before.load(Ordering::Acquire), value_before + 1);
        assert_eq!(probe.value_after.load(Ordering::Acquire), value_before + 1);

        if case_id.contains("n_string") || case_id.contains("scalar_getters") {
            assert_eq!(
                result_set.n_string(label).expect("n-string").as_deref(),
                Some("value")
            );
        }
        if case_id.contains("object") {
            assert_eq!(result_set.object(id).expect("object"), Value::Int(7).into());
        }
        if case_id.contains("metadata") {
            let before = probe.metadata_before.load(Ordering::Acquire);
            assert!(result_set.meta_data().is_ok());
            assert_eq!(probe.metadata_before.load(Ordering::Acquire), before + 1);
            assert_eq!(probe.metadata_after.load(Ordering::Acquire), before + 1);
        }
        if case_id.contains("resource_getters") {
            assert!(result_set
                .binary_stream(label)
                .expect("binary stream")
                .is_some());
        }
        if case_id.contains("update") || case_id.contains("row_mutation") {
            let update = result_set
                .update_value(label, ResultSetUpdate::NString(Some("updated".to_owned())));
            assert_eq!(
                update,
                Err(druid_rdbc::error::RdbcError::UnsupportedOperation {
                    operation: "result_set_update_value"
                }),
                "SQLite read-only ResultSet must reject typed update for case {case_id}"
            );
        }
        if case_id.contains("navigation") {
            // 固定BASE真实Toasty缓冲结果支持导航；首行向前移到首行前，再恢复定位。
            assert!(!result_set.previous().await.expect("previous before first"));
            assert_eq!(result_set.row().expect("before-first row"), 0);
            assert!(result_set.is_before_first().expect("before first"));
            assert!(result_set.next().await.expect("restore first row"));
        }
        assert_eq!(result_set.value(absent).expect("null"), Value::Null);
        assert!(result_set.was_null().expect("was_null"));

        let close_before = probe.close_before.load(Ordering::Acquire);
        result_set.close().await.expect("close");
        assert_eq!(probe.close_before.load(Ordering::Acquire), close_before + 1);
        assert_eq!(probe.close_after.load(Ordering::Acquire), close_before + 1);
        statement.close().await.expect("statement close");
        executed.push(*case_id);
    }

    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_result_set_filter_canonical_matrix:{}",
        executed.join("|")
    );
    Connection::close(&mut connection)
        .await
        .expect("lease close");
}
