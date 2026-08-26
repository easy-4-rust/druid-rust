use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::Value;
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn toasty_direct_methods_survive_owned_statement_and_transaction_boundaries() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    connection
        .exec(
            "create table item(id integer primary key autoincrement, value text not null)",
            Vec::new(),
        )
        .await
        .unwrap();

    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();

    assert_eq!(
        connection
            .exec_batch(vec![
                (
                    "insert into item(value) values (?)".to_owned(),
                    vec![Value::String("first".to_owned())],
                ),
                (
                    "insert into item(value) values (?)".to_owned(),
                    vec![Value::String("second".to_owned())],
                ),
            ])
            .await
            .unwrap(),
        vec![1, 1]
    );
    let inserted = connection
        .exec(
            "insert into item(value) values (?)",
            vec![Value::String("third".to_owned())],
        )
        .await
        .unwrap();
    assert_eq!(inserted.rows_affected, 1);
    assert_eq!(inserted.last_insert_id, Some(3));
    assert_eq!(
        connection
            .fetch("select value from item order by id", Vec::new())
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.values)
            .collect::<Vec<_>>(),
        vec![
            vec![Value::String("first".to_owned())],
            vec![Value::String("second".to_owned())],
            vec![Value::String("third".to_owned())],
        ]
    );

    connection.set_auto_commit(false).await.unwrap();
    let savepoint = connection.set_savepoint_named("safe_1").await.unwrap();
    connection
        .exec("insert into item(value) values ('rolled_back')", Vec::new())
        .await
        .unwrap();
    connection.rollback_to(&savepoint).await.unwrap();
    connection.release_savepoint(&savepoint).await.unwrap();
    connection.commit().await.unwrap();
    assert_eq!(connection.auto_commit(), Ok(true));
    assert!(connection.ping().await.is_ok());
    assert_eq!(connection.warnings().await.unwrap(), None);
    connection.clear_warnings().await.unwrap();
    assert!(connection
        .fetch(
            "select value from item where value = 'rolled_back'",
            Vec::new(),
        )
        .await
        .unwrap()
        .is_empty());

    connection.close().await.unwrap();
    assert_eq!(connection.is_closed(), Ok(true));
}
