use chrono::{NaiveDate, NaiveDateTime};
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;

fn timestamp(hour: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 9)
        .unwrap()
        .and_hms_micro_opt(hour, 34, 56, 123_456)
        .unwrap()
}

/// 在显式配置的独立 MySQL 实库检查会话时间、微秒、NULL 和缓冲/流式解码。
#[tokio::test]
async fn timestamp_preserves_mysql_session_fields_and_datetime_does_not_shift() {
    if std::env::var("DRUID_MYSQL_TIMESTAMP_CONTRACT_REQUIRED").as_deref() != Ok("1") {
        return;
    }
    let url = std::env::var("DRUID_DATABASE_URL").expect("显式实库合同必须配置 URL");
    let mut connection = SqlxConnectionAdapter::connect(&url).await.unwrap();
    connection
        .exec("SET time_zone = '+08:00'", Vec::new())
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TEMPORARY TABLE druid_timestamp_session_contract (id INT PRIMARY KEY, dt DATETIME(6), ts TIMESTAMP(6) NULL)",
            Vec::new(),
        )
        .await
        .unwrap();
    connection
        .exec(
            "INSERT INTO druid_timestamp_session_contract VALUES (1, ?, ?), (2, NULL, NULL)",
            vec![
                Value::Timestamp(timestamp(12)),
                Value::Timestamp(timestamp(12)),
            ],
        )
        .await
        .unwrap();

    for (zone, hour) in [("+08:00", 12), ("+00:00", 4), ("-02:00", 2)] {
        connection
            .exec(&format!("SET time_zone = '{zone}'"), Vec::new())
            .await
            .unwrap();
        let rows = connection
            .fetch(
                "SELECT dt, ts FROM druid_timestamp_session_contract ORDER BY id",
                Vec::new(),
            )
            .await
            .expect("TIMESTAMP 不能被 DATETIME 类型兼容检查拒绝");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].values,
            vec![
                Value::Timestamp(timestamp(12)),
                Value::Timestamp(timestamp(hour))
            ]
        );
        assert_eq!(rows[1].values, vec![Value::Null, Value::Null]);

        for fetch_size in [0, 1] {
            let mut statement = connection
                .prepare_statement(
                    "SELECT dt, ts FROM druid_timestamp_session_contract ORDER BY id",
                    StatementOptions::default(),
                )
                .await
                .unwrap();
            statement.set_fetch_size(fetch_size).unwrap();
            let mut result = statement.execute_prepared_query().await.unwrap();
            assert!(result.next().await.unwrap());
            assert_eq!(
                result.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::Timestamp(timestamp(12))
            );
            assert_eq!(
                result.value(ColumnIndex::new(2).unwrap()).unwrap(),
                Value::Timestamp(timestamp(hour))
            );
            assert!(result.next().await.unwrap());
            assert_eq!(
                result.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::Null
            );
            assert_eq!(
                result.value(ColumnIndex::new(2).unwrap()).unwrap(),
                Value::Null
            );
            assert!(!result.next().await.unwrap());
            result.close().await.unwrap();
            statement.close().await.unwrap();
        }
    }
    connection.close().await.unwrap();
}
