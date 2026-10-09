//! 真实 SQLite 验证缓冲游标的原始标签、缺失标签和生命周期合同。

use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn buffered_missing_labels_preserve_invalid_argument_and_lease_priority() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement.execute_query("SELECT 3 AS id").await.unwrap();
    assert_eq!(rows.find_column("id"), Ok(ColumnIndex::new(1).unwrap()));
    assert_eq!(
        rows.find_column("missing"),
        Err(RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    // 非原生合成标签仍不能用于实际列查找。
    assert!(matches!(
        rows.find_column("column_1"),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 3);
    assert_eq!(rows.int_by_label("id"), Ok(3));
    assert_eq!(
        rows.int_by_label("missing"),
        Err(RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    connection.close().await.unwrap();
    assert!(matches!(
        rows.find_column("id"),
        Err(RdbcError::ConnectionDiscarded)
    ));
    rows.close().await.unwrap();
    assert!(matches!(
        rows.int_by_label("id"),
        Err(RdbcError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn buffered_generated_keys_keep_real_labels_even_without_rows() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    connection
        .exec("CREATE TABLE keys_t(id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut empty = statement.generated_keys().await.unwrap();
    assert_eq!(
        empty.find_column("generated_key").unwrap(),
        ColumnIndex::new(1).unwrap()
    );
    assert!(!empty.next().await.unwrap());
    empty.close().await.unwrap();
    statement
        .execute_update(
            "INSERT INTO keys_t VALUES (3)",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let mut keys = statement.generated_keys().await.unwrap();
    assert_eq!(
        keys.find_column("generated_key").unwrap(),
        ColumnIndex::new(1).unwrap()
    );
    assert!(keys.next().await.unwrap());
    assert_eq!(keys.long_by_label("GENERATED_KEY").unwrap(), 3);
    assert!(matches!(
        keys.find_column("missing"),
        Err(RdbcError::InvalidArgument(_))
    ));
    keys.close().await.unwrap();
    assert!(matches!(
        keys.find_column("generated_key"),
        Err(RdbcError::ConnectionDiscarded)
    ));
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
