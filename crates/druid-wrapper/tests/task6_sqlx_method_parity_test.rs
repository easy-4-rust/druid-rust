use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnNullability, DatabaseMetaDataInfoExt, DatabaseMetaDataLimitsExt,
    DatabaseMetaDataQueryExt, ResultSetColumnType,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;

#[tokio::test]
async fn sqlx_restores_savepoint_statement_prepared_and_driver_metadata_semantics() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec(
            "create table item(id integer primary key autoincrement, name text not null, note text)",
            Vec::new(),
        )
        .await
        .unwrap();

    connection.set_auto_commit(false).await.unwrap();
    connection
        .exec("insert into item(name) values ('before')", Vec::new())
        .await
        .unwrap();
    let savepoint = connection.set_savepoint_named("safe_1").await.unwrap();
    connection
        .exec("insert into item(name) values ('rolled_back')", Vec::new())
        .await
        .unwrap();
    connection.rollback_to(&savepoint).await.unwrap();
    connection.release_savepoint(&savepoint).await.unwrap();
    connection.commit().await.unwrap();
    assert!(connection.set_savepoint_named("unsafe-name").await.is_err());
    assert!(!connection.read_only().unwrap());
    assert!(connection.set_read_only(true).await.is_err());
    assert_eq!(connection.warnings().await.unwrap(), None);
    connection.clear_warnings().await.unwrap();

    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .add_batch("insert into item(name) values ('batch_a')".to_owned())
        .unwrap();
    statement
        .add_batch("insert into item(name) values ('batch_b')".to_owned())
        .unwrap();
    assert_eq!(statement.execute_batch().await.unwrap(), vec![1, 1]);
    let results = statement
        .execute(
            "select id, name, note from item order by id",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut rows = match results.into_iter().next().unwrap() {
        ExecutionResult::ResultSet(rows) => rows,
        ExecutionResult::UpdateCount(_) => panic!("driver describe must classify SELECT as rows"),
    };
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.string_by_label("name").unwrap().as_deref(),
        Some("before")
    );
    let metadata = rows.meta_data().unwrap();
    let id = ColumnIndex::new(1).unwrap();
    let name = ColumnIndex::new(2).unwrap();
    let note = ColumnIndex::new(3).unwrap();
    assert_ne!(metadata.column_type_name(id).unwrap(), "UNKNOWN");
    assert!(matches!(
        metadata.column_type(id).unwrap(),
        ResultSetColumnType::Jdbc(_) | ResultSetColumnType::Integer
    ));
    assert_eq!(
        metadata.nullability(name).unwrap(),
        ColumnNullability::NoNulls
    );
    assert_eq!(
        metadata.nullability(note).unwrap(),
        ColumnNullability::Nullable
    );

    statement
        .execute_update(
            "insert into item(name) values ('generated')",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let mut generated = statement.generated_keys().await.unwrap();
    assert!(generated.next().await.unwrap());
    assert!(generated.long(ColumnIndex::new(1).unwrap()).unwrap() > 0);

    let mut prepared = connection
        .prepare_statement(
            "insert into item(name, note) values (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String("prepared_a".to_owned()),
            },
            PreparedParameter {
                index: ParameterIndex::new(2).unwrap(),
                value: Value::Null,
            },
        ])
        .unwrap();
    prepared
        .add_parameter_batch(vec![
            PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::String("prepared_b".to_owned()),
            },
            PreparedParameter {
                index: ParameterIndex::new(2).unwrap(),
                value: Value::String("note".to_owned()),
            },
        ])
        .unwrap();
    assert_eq!(prepared.execute_prepared_batch().await.unwrap(), vec![1, 1]);

    let prepared_query = connection
        .prepare_statement(
            "select id, name, note from item",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    let prepared_metadata = prepared_query.result_set_meta_data().unwrap();
    assert_eq!(prepared_metadata.column_count(), 3);
    assert_ne!(
        prepared_metadata
            .column_type_name(ColumnIndex::new(1).unwrap())
            .unwrap(),
        "UNKNOWN"
    );
}

#[tokio::test]
async fn sqlx_unsupported_metadata_families_fail_closed_with_typed_errors() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut metadata = connection.database_meta_data().await.unwrap();

    let error = match metadata.get_functions(None, None, None).await {
        Ok(_) => panic!("SQLite function metadata must remain explicitly unsupported"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RdbcError::UnsupportedOperation {
            operation: "get_functions"
        }
    ));
}

#[tokio::test]
async fn sqlx_exposes_configured_table_column_key_type_and_capability_metadata() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    connection
        .exec(
            "create table parent(id integer primary key, name text not null)",
            Vec::new(),
        )
        .await
        .unwrap();
    connection
        .exec(
            "create table child(id integer primary key, parent_id integer references parent(id))",
            Vec::new(),
        )
        .await
        .unwrap();
    connection
        .exec(
            "create unique index parent_name_uq on parent(name)",
            Vec::new(),
        )
        .await
        .unwrap();

    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_eq!(
        metadata.get_url().await.unwrap().as_deref(),
        Some("sqlite::memory:")
    );
    assert_eq!(
        metadata.get_driver_name().await.unwrap().as_deref(),
        Some("sqlx")
    );
    assert!(metadata.supports_transactions().await.unwrap());
    assert!(metadata.supports_batch_updates().await.unwrap());
    assert!(metadata.supports_savepoints().await.unwrap());
    assert!(metadata.supports_named_parameters().await.unwrap());

    let table_types = vec!["TABLE".to_owned()];
    let mut tables = metadata
        .get_tables(None, None, Some("parent"), Some(&table_types))
        .await
        .unwrap();
    assert!(tables.next().await.unwrap());
    assert_eq!(
        tables.string_by_label("TABLE_NAME").unwrap().as_deref(),
        Some("parent")
    );

    let mut columns = metadata
        .get_columns(None, None, Some("parent"), Some("%"))
        .await
        .unwrap();
    assert!(columns.next().await.unwrap());
    assert_eq!(
        columns.string_by_label("COLUMN_NAME").unwrap().as_deref(),
        Some("id")
    );
    assert_ne!(
        columns.string_by_label("TYPE_NAME").unwrap().as_deref(),
        Some("UNKNOWN")
    );
    let mut wildcard_columns = metadata
        .get_columns(None, None, Some("parent"), Some("NA_E"))
        .await
        .unwrap();
    assert!(wildcard_columns.next().await.unwrap());
    assert_eq!(
        wildcard_columns
            .string_by_label("COLUMN_NAME")
            .unwrap()
            .as_deref(),
        Some("name")
    );
    let mut escaped_columns = metadata
        .get_columns(None, None, Some("parent"), Some(r"na\_e"))
        .await
        .unwrap();
    assert!(!escaped_columns.next().await.unwrap());

    let mut primary_keys = metadata
        .get_primary_keys(None, None, Some("parent"))
        .await
        .unwrap();
    assert!(primary_keys.next().await.unwrap());
    assert_eq!(
        primary_keys
            .string_by_label("COLUMN_NAME")
            .unwrap()
            .as_deref(),
        Some("id")
    );

    let mut imported_keys = metadata
        .get_imported_keys(None, None, Some("child"))
        .await
        .unwrap();
    assert!(imported_keys.next().await.unwrap());
    assert_eq!(
        imported_keys
            .string_by_label("PKTABLE_NAME")
            .unwrap()
            .as_deref(),
        Some("parent")
    );

    let mut exported_keys = metadata
        .get_exported_keys(None, None, Some("parent"))
        .await
        .unwrap();
    assert!(exported_keys.next().await.unwrap());
    assert_eq!(
        exported_keys
            .string_by_label("FKTABLE_NAME")
            .unwrap()
            .as_deref(),
        Some("child")
    );

    let mut index_info = metadata
        .get_index_info(None, None, Some("parent"), true, false)
        .await
        .unwrap();
    assert!(index_info.next().await.unwrap());
    assert_eq!(
        index_info.string_by_label("INDEX_NAME").unwrap().as_deref(),
        Some("parent_name_uq")
    );
    assert_eq!(
        index_info
            .string_by_label("COLUMN_NAME")
            .unwrap()
            .as_deref(),
        Some("name")
    );

    let mut type_info = metadata.get_type_info().await.unwrap();
    assert!(type_info.next().await.unwrap());
    assert!(type_info.string_by_label("TYPE_NAME").unwrap().is_some());
}
