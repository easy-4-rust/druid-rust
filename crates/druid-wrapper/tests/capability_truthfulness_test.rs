//! 以真实 SQLite 操作校验 adapter 宣称支持的事务和保存点能力。
#![cfg(all(feature = "sqlx", feature = "toasty-sqlite"))]

use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::types::{Row, Value};
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn sqlite_backends_only_advertise_exercised_connection_capabilities() {
    let factories: Vec<Box<dyn ConnectionFactory>> = vec![
        Box::new(SqlxConnectionFactory::new("sqlite::memory:")),
        Box::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .expect("create Toasty SQLite factory"),
        ),
    ];

    for factory in factories {
        let backend = factory.name();
        let mut connection = factory.create().await.expect("connect to real SQLite");
        let capabilities = connection.capabilities();
        assert!(
            capabilities.transactions,
            "{backend} must advertise transactions"
        );
        assert!(
            capabilities.savepoints,
            "{backend} must advertise savepoints"
        );
        assert!(
            !capabilities.catalogs,
            "{backend} must not advertise catalogs"
        );
        assert!(
            !capabilities.schemas,
            "{backend} must not advertise schemas"
        );
        assert!(
            !capabilities.callable_statements,
            "{backend} must not advertise callable statements"
        );

        connection
            .exec(
                "CREATE TABLE capability_probe (value INTEGER NOT NULL)",
                vec![],
            )
            .await
            .expect("create probe table");
        connection
            .set_auto_commit(false)
            .await
            .expect("begin transaction");
        connection
            .exec("INSERT INTO capability_probe VALUES (1)", vec![])
            .await
            .expect("insert before savepoint");
        let savepoint = connection
            .set_savepoint_named("capability_probe_sp")
            .await
            .expect("create savepoint");
        connection
            .exec("INSERT INTO capability_probe VALUES (2)", vec![])
            .await
            .expect("insert after savepoint");
        connection
            .rollback_to(&savepoint)
            .await
            .expect("roll back to savepoint");
        connection
            .release_savepoint(&savepoint)
            .await
            .expect("release savepoint");
        connection.commit().await.expect("commit transaction");
        connection
            .set_auto_commit(true)
            .await
            .expect("restore auto commit");
        assert_eq!(
            connection
                .fetch("SELECT value FROM capability_probe", vec![])
                .await
                .expect("query committed rows"),
            vec![Row::new(vec![Value::Int(1)])],
            "{backend} must preserve only the row before the savepoint"
        );

        connection
            .set_auto_commit(false)
            .await
            .expect("begin rollback transaction");
        connection
            .exec("INSERT INTO capability_probe VALUES (3)", vec![])
            .await
            .expect("insert rollback candidate");
        connection.rollback().await.expect("roll back transaction");
        connection
            .set_auto_commit(true)
            .await
            .expect("restore auto commit after rollback");
        assert_eq!(
            connection
                .fetch("SELECT value FROM capability_probe", vec![])
                .await
                .expect("query rows after rollback"),
            vec![Row::new(vec![Value::Int(1)])],
            "{backend} must roll back the whole second transaction"
        );
        connection.close().await.expect("close connection");
    }
}
