//! SQLx Statement/PreparedStatement must own an executor beyond the create-call borrow.

use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;

#[tokio::test]
async fn statement_and_result_set_keep_the_sqlx_connection_lease_owned() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("owned statement");
    drop(connection);

    statement
        .execute_update(
            "create table owned_item(id integer primary key, name text not null)",
            GeneratedKeys::None,
        )
        .await
        .expect("create table after create-call borrow ended");
    statement
        .execute_update(
            "insert into owned_item(id, name) values (1, 'alpha')",
            GeneratedKeys::None,
        )
        .await
        .expect("insert through statement-owned actor");
    let mut result_set = statement
        .execute_query("select name from owned_item where id = 1")
        .await
        .expect("owned result set");
    drop(statement);

    assert!(result_set.next().await.expect("advance owned result set"));
    assert_eq!(
        result_set
            .value(ColumnIndex::new(1).expect("1-based index"))
            .expect("current value"),
        Value::String("alpha".to_owned())
    );
    result_set.close().await.expect("close owned result set");
}

#[tokio::test]
async fn prepared_statement_materializes_standard_parameters_through_the_owned_actor() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let mut prepared =
        Connection::prepare_statement(&mut connection, "select ?", StatementOptions::default())
            .await
            .expect("owned prepared statement");
    prepared
        .set_parameter(PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("1-based index"),
            value: Value::String("owned".to_owned()),
        })
        .expect("set standard parameter");
    drop(connection);

    let mut result_set = prepared
        .execute_prepared_query()
        .await
        .expect("prepared query after connection borrow ended");
    drop(prepared);
    assert!(result_set.next().await.expect("advance result set"));
    assert_eq!(
        result_set
            .value(ColumnIndex::new(1).expect("1-based index"))
            .expect("prepared value"),
        Value::String("owned".to_owned())
    );
}
