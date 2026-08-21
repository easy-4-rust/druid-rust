//! 外部池与直连 `RDBC` 连接的共同可观察合同。

use druid::core::{DruidError, DruidPooledConnection, PhysicalConnection, Value};
use std::future::Future;

/// 断言获取的连接具备直连 SQLx 已声明的基础 `RDBC` 能力。
///
/// 该合同通过真实 SQLite 验证 PreparedStatement、Metadata、幂等关闭，以及关闭后
/// 再次获取的可用性。参数 `acquire` 每次调用都必须从同一个连接拥有者获取连接。
pub async fn assert_rdbc_connection_contract<F, Fut>(acquire: F) -> Result<(), DruidError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, DruidError>>,
{
    let mut connection = acquire().await?;
    connection
        .exec(
            "create table item(id integer primary key, name text)",
            vec![],
        )
        .await?;
    let mut prepared = connection
        .prepare_statement("insert into item(name) values (?)")
        .await?;
    let result = prepared
        .exec(&mut connection, vec![Value::String("alpha".to_owned())])
        .await?;
    assert_eq!(result.rows_affected, 1);

    let mut metadata = connection.get_meta_data()?;
    assert_eq!(
        metadata.get_database_product_name().await?.as_deref(),
        Some("SQLite")
    );
    drop(metadata);

    connection.close().await?;
    connection.close().await?;

    let mut reused = acquire().await?;
    let rows = reused
        .fetch("select name from item where id = 1", vec![])
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, vec![Value::String("alpha".to_owned())]);
    reused.close().await?;
    Ok(())
}
