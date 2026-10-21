//! 结果集诊断：stream_and_stat 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 恢复原22个流更新重载、共享读位置与真实SQLite能力错误。
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn task6_pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors() {
    use super::stream_update_short_circuit_filter::StreamUpdateShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StreamUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        95,
        "sqlite-stream-update-short-circuit",
        Arc::new(filter_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();
    let ascii = RdbcInputStream::from_bytes([1, 2, 3]);
    let binary = RdbcInputStream::from_bytes([4, 5, 6]);
    let character = RdbcReader::from_string("甲乙");
    let n_character = RdbcReader::from_string("丙丁");

    result_set
        .update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Int(-7),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(i32::MAX),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Long(-11),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Int(i32::MIN),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(13),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Long(-17),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(19),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Int(-23),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(29),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Long(-31),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(37),
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
        )
        .unwrap();
    result_set
        .update_value(
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Long(-41),
            },
        )
        .unwrap();
    result_set
        .update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(43),
            },
        )
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_ascii_stream:1:some",
            "result_set_update_ascii_stream_by_label:ascii:none",
            "result_set_update_ascii_stream_with_int_length:2:some:-7",
            "result_set_update_ascii_stream_by_label_with_int_length:ascii_int:none:2147483647",
            "result_set_update_ascii_stream_with_length:3:some:-11",
            "result_set_update_ascii_stream_by_label_with_length:ascii_long:none:9223372036854775807",
            "result_set_update_binary_stream:4:some",
            "result_set_update_binary_stream_by_label:binary:none",
            "result_set_update_binary_stream_with_int_length:5:some:-2147483648",
            "result_set_update_binary_stream_by_label_with_int_length:binary_int:none:13",
            "result_set_update_binary_stream_with_length:6:some:-17",
            "result_set_update_binary_stream_by_label_with_length:binary_long:none:19",
            "result_set_update_character_stream:7:some",
            "result_set_update_character_stream_by_label:character:none",
            "result_set_update_character_stream_with_int_length:8:some:-23",
            "result_set_update_character_stream_by_label_with_int_length:character_int:none:29",
            "result_set_update_character_stream_with_length:9:some:-31",
            "result_set_update_character_stream_by_label_with_length:character_long:none:37",
            "result_set_update_n_character_stream:10:some",
            "result_set_update_n_character_stream_by_label:n_character:none",
            "result_set_update_n_character_stream_with_length:11:some:-41",
            "result_set_update_n_character_stream_by_label_with_length:n_character_long:none:43",
        ]
    );
    assert_eq!(ascii.read_to_end().unwrap(), [2, 3]);
    assert_eq!(binary.read_to_end().unwrap(), [5, 6]);
    assert_eq!(character.read_to_string().unwrap(), "乙");
    assert_eq!(n_character.read_to_string().unwrap(), "丁");

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_value_by_label(
            "fail",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(-47)
            }
        ),
        Err(RdbcError::DriverError(
            "filtered stream update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory,
        96,
        "sqlite-stream-update-pass-through",
        Arc::new(pass_chain),
    )
    .await;
    let mut result_set = statement
        .execute_query("SELECT X'010203' AS value")
        .await
        .unwrap();

    macro_rules! assert_stream_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(RdbcError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 对 22 个精确 Filter
    // 入口保持 generic update capability error，且每次失败只计数一次。
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Int(-7)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(i32::MAX)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&ascii).cloned(),
                length: RdbcStreamLength::Long(-11)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Long(i64::MAX)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Int(i32::MIN)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(13)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&binary).cloned(),
                length: RdbcStreamLength::Long(-17)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Long(19)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Int(-23)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(29)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&character).cloned(),
                length: RdbcCharacterLength::Long(-31)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(37)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Unspecified
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_value(
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&n_character).cloned(),
                length: RdbcCharacterLength::Long(-41)
            }
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(43)
            }
        ),
        "result_set_update_value_by_label"
    );
    assert_eq!(statement.exception_count(), 22);
    assert!(!result_set.is_closed().unwrap());
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 恢复完整原真实SQLite读取、统计与创建语句级联关闭合同。
#[tokio::test]
async fn task6_real_sqlite_result_sets_flow_through_stat_filter_and_statement_cascade() {
    use super::native_result_set_close_observer::NativeResultSetCloseObserver;
    use crate::core::FilterChain;
    use crate::stats::{StatFilter, StatsCollector};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument, RdbcTargetType};
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;
    let collector = Arc::new(StatsCollector::new("sqlite-stat", Duration::from_secs(1)));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(stat_filter);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let (mut first_statement, mut connection) =
        task6_sqlite_metadata_statement(&factory, 72, "sqlite-stat", Arc::new(filter_chain)).await;
    let mut first = first_statement
        .execute_query(
            "SELECT 1 AS n, '12.340' AS decimal_value, '2025-01-02' AS date_value, \
             '03:04:05' AS time_value, '2025-01-02 03:04:05' AS timestamp_value \
             UNION ALL SELECT 2, '56.780', '2025-02-03', '06:07:08', \
             '2025-02-03 06:07:08' UNION ALL SELECT 3, '90.120', '2025-03-04', \
             '09:10:11', '2025-03-04 09:10:11'",
        )
        .await
        .unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 1);
    assert!(first.next().await.unwrap());
    assert_eq!(first.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(
        first.object(ColumnIndex::new(1).unwrap()).unwrap(),
        RdbcObject::Scalar(Value::Int(1))
    );
    assert_eq!(
        first
            .object_as(ColumnIndex::new(1).unwrap(), &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Long(1)
    );
    assert_eq!(
        first.big_decimal(ColumnIndex::new(2).unwrap()).unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        first
            .date(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        first
            .time(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        first
            .timestamp(
                ColumnIndex::new(5).unwrap(),
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert!(first.next().await.unwrap());
    first.close().await.unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 0);
    assert_eq!(collector.result_set_stat().fetch_row_count(), 2);
    assert_eq!(collector.result_set_stat().close_count(), 1);

    first_statement.close().await.unwrap();
    assert_eq!(collector.result_set_stat().close_count(), 1);

    // 第二语句来自同一真实连接，复用同一策略、资源登记和ID种子。
    let admission = first_statement.inner.begin_standard_admission().unwrap();
    let raw = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut second_statement = DruidPooledStatement::new_standard(
        raw,
        StatementOptions::default(),
        Arc::clone(&first_statement.inner.resources),
        Arc::clone(&first_statement.inner.policy),
        72,
        2,
        Arc::clone(&first_statement.inner.result_set_id_seed),
        Arc::clone(&first_statement.inner.metadata_id_seed),
        Arc::clone(&first_statement.inner.lease_active),
        first_statement.inner.filter_chain.clone(),
        None,
    );
    first_statement
        .inner
        .resources
        .register(&admission, Arc::clone(&second_statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut second = second_statement
        .execute_query("SELECT 4 UNION ALL SELECT 5")
        .await
        .unwrap();
    // 包围原查询登记的同一原生游标，只观察实际关闭，不能合成关闭状态。
    let native_closed = Arc::new(AtomicBool::new(false));
    let native_close_count = Arc::new(AtomicUsize::new(0));
    let resource = second_statement.current_result_set_for_diagnostics();
    {
        let mut slot = resource.result_set.lock().await;
        let original = slot.take().expect("同一真实SQLite查询游标");
        let identity = std::ptr::from_ref::<dyn ResultSet>(original.as_ref()).cast::<()>() as usize;
        *slot = Some(Box::new(NativeResultSetCloseObserver::new(
            original,
            Arc::clone(&native_closed),
            Arc::clone(&native_close_count),
            identity,
        )));
    }
    assert!(second.next().await.unwrap());
    second_statement.close().await.unwrap();

    let stat = collector.result_set_stat();
    assert_eq!(stat.open_count(), 2);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.opening_max(), 1);
    assert_eq!(stat.fetch_row_count(), 3);
    assert_eq!(stat.close_count(), 2);
    assert!(stat.last_open_time_millis().is_some());
    assert!(second.is_closed().unwrap());
    assert!(native_closed.load(Ordering::Acquire));
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    second_statement.close().await.unwrap();
    second.close().await.unwrap();
    assert_eq!(native_close_count.load(Ordering::Acquire), 1);
    assert_eq!(stat.close_count(), 2);
    connection.close().await.unwrap();
}
