//! 整数结果处置必须保留真实 Druid callable 代理的关闭与租约错误顺序。

use druid::core::{DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{CallableStatement, PreparedStatement, Statement, StatementOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 提供可关闭 callable 资源的测试驱动连接。
struct CallableOwnerConnection {
    closes: Arc<AtomicUsize>,
}

druid_rdbc::impl_standard_wrapper!(CallableOwnerConnection => Connection);

#[async_trait::async_trait]
impl Connection for CallableOwnerConnection {
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(CallableOwnerResource {
            closed: false,
            closes: self.closes.clone(),
            value: druid_rdbc::types::Value::Int(0),
        }))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

/// 记录实际底层关闭次数，不替代 Druid 逻辑代理的错误检查。
struct CallableOwnerResource {
    closed: bool,
    closes: Arc<AtomicUsize>,
    value: druid_rdbc::types::Value,
}

druid_rdbc::impl_standard_wrapper!(CallableOwnerResource => CallableStatement);

#[async_trait::async_trait]
impl Statement for CallableOwnerResource {
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.closes.fetch_add(1, Ordering::SeqCst);
            self.closed = true;
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for CallableOwnerResource {
    fn set_parameter(
        &mut self,
        parameter: druid_rdbc::statement::PreparedParameter,
    ) -> Result<(), RdbcError> {
        self.value = parameter.value;
        Ok(())
    }

    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        Ok(Box::new(
            druid_rdbc::extensions::BufferedRowSet::with_column_labels(
                vec![druid_rdbc::types::Row::new(vec![self.value.clone()])],
                vec!["value".to_owned()],
            ),
        ))
    }
}

#[async_trait::async_trait]
impl CallableStatement for CallableOwnerResource {}

#[tokio::test]
async fn result_statement_keeps_callable_leaf_and_shared_close_after_original_drop() {
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::{ParameterIndex, PreparedParameter};
    use druid_rdbc::types::Value;
    use druid_rdbc::wrapper::StandardUnwrappedMut;
    use std::any::TypeId;

    let closes = Arc::new(AtomicUsize::new(0));
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(CallableOwnerConnection {
            closes: closes.clone(),
        }),
        99,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let mut connection = DruidPooledConnection::with_holder(
        holder,
        "callable-result-owner".to_owned(),
        None,
        false,
        Box::new(|_, _| false),
    );
    let mut original = connection
        .prepare_call("{call sample(?)}", StatementOptions::default())
        .await
        .unwrap();
    original
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(7),
        })
        .unwrap();
    let rows = original.execute_prepared_query().await.unwrap();
    let mut owner = rows
        .get_statement()
        .expect("Callable 结果必须支持标准创建语句入口")
        .unwrap();
    let StandardUnwrappedMut::CallableStatement(leaf) = owner
        .unwrap_mut(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap()
    else {
        panic!("必须保留 Callable 叶子接口");
    };
    leaf.set_parameter(PreparedParameter {
        index: ParameterIndex::new(1).unwrap(),
        value: Value::Int(9),
    })
    .unwrap();
    let mut changed = original.execute_prepared_query().await.unwrap();
    assert!(changed.next().await.unwrap());
    assert_eq!(
        changed.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(9)
    );
    drop(original);
    drop(rows);
    let StandardUnwrappedMut::CallableStatement(leaf) = owner
        .unwrap_mut(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap()
    else {
        panic!("必须保留 Callable 叶子接口");
    };
    let mut surviving = leaf.execute_prepared_query().await.unwrap();
    assert!(surviving.next().await.unwrap());
    assert_eq!(
        surviving.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Int(9)
    );
    owner.close().await.unwrap();
    assert!(changed.is_closed().unwrap());
    assert!(surviving.is_closed().unwrap());
    owner.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn mutable_callable_unwrap_closes_the_same_logical_owner_once() {
    use druid_rdbc::wrapper::StandardUnwrappedMut;
    use std::any::TypeId;

    let closes = Arc::new(AtomicUsize::new(0));
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(CallableOwnerConnection {
            closes: closes.clone(),
        }),
        98,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let mut connection = DruidPooledConnection::with_holder(
        holder,
        "callable-mutable-owner".to_owned(),
        None,
        false,
        Box::new(|_, _| false),
    );
    let mut callable = connection
        .prepare_call("{call sample()}", StatementOptions::default())
        .await
        .unwrap();
    let expected = std::ptr::from_ref(callable.as_ref()).cast::<()>();
    let wrapper: &mut dyn druid_rdbc::wrapper::Wrapper = callable.as_mut();
    let StandardUnwrappedMut::CallableStatement(view) = wrapper
        .unwrap_mut(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap()
    else {
        panic!("必须保留可调用语句的可变叶子接口");
    };
    assert_eq!(std::ptr::from_mut(view).cast::<()>().cast_const(), expected);
    assert_eq!(view.sql().unwrap(), "{call sample()}");
    view.close().await.unwrap();
    assert!(callable.is_closed().unwrap());
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    callable.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn callable_integer_disposition_checks_logical_owner_before_invalid_argument() {
    for close_connection in [false, true] {
        let closes = Arc::new(AtomicUsize::new(0));
        let holder = DruidConnectionHolder::with_standard_connection(
            Box::new(CallableOwnerConnection {
                closes: closes.clone(),
            }),
            97,
            Duration::ZERO,
            0,
        )
        .await
        .unwrap();
        let mut connection = DruidPooledConnection::with_holder(
            holder,
            "callable-current-owner".to_owned(),
            None,
            false,
            Box::new(|_, _| false),
        );
        let mut callable = connection
            .prepare_call("{call sample()}", StatementOptions::default())
            .await
            .unwrap();
        assert!(matches!(
            callable.get_more_results_current(999).await,
            Err(RdbcError::InvalidArgument(_))
        ));
        assert_eq!(closes.load(Ordering::SeqCst), 0);
        if close_connection {
            connection.close().await.unwrap();
        } else {
            callable.close().await.unwrap();
        }
        assert_eq!(closes.load(Ordering::SeqCst), 1);
        for current in [999, 0, -1, 1, 2, 3] {
            assert!(matches!(callable.get_more_results_current(current).await, Err(RdbcError::ConnectionDiscarded)),
                "callable 关闭/租约失效先于参数错误，connection_closed={close_connection}, current={current}");
        }
        callable.close().await.unwrap();
        connection.close().await.unwrap();
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }
}
