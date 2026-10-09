use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableParameter, CallableStatement, ParameterIndex, PreparedStatement, Statement,
};
use druid_rdbc::types::Value;

/// 独立标准驱动默认索引校验控制，只实现真实关闭状态与原 OUT 值。
/// 对应 Java: CallableStatement 参数索引校验；不实现 Druid 池化策略。
struct DefaultIndexCallable {
    closed: bool,
}
druid_rdbc::impl_standard_wrapper!(DefaultIndexCallable => CallableStatement);
#[async_trait::async_trait]
impl Statement for DefaultIndexCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
#[async_trait::async_trait]
impl PreparedStatement for DefaultIndexCallable {}
#[async_trait::async_trait]
impl CallableStatement for DefaultIndexCallable {
    fn out_value(&self, index: ParameterIndex) -> Result<Value, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let value = i64::try_from(index.get())
            .map_err(|_| RdbcError::InvalidArgument("index exceeds test OUT integer".to_owned()))?;
        Ok(Value::Int(value))
    }
}

/// 默认校验与原严格 selector 构造等价，有效索引没有换成命名参数。
#[tokio::test]
async fn callable_default_index_validation_preserves_checked_selector_and_original_out() {
    let mut callable = DefaultIndexCallable { closed: false };
    assert!(
        matches!(callable.index_parameter(0), Err(RdbcError::InvalidArgument(message)) if message == "parameter index is 1-based")
    );
    assert!(CallableParameter::by_index(0).is_err());
    let first = callable.index_parameter(1).unwrap();
    assert_eq!(first, CallableParameter::by_index(1).unwrap());
    assert_eq!(
        callable.out_parameter(&first).unwrap(),
        druid_rdbc::types::RdbcObject::from(Value::Int(1))
    );
    assert_eq!(
        callable.index_parameter(usize::MAX).unwrap(),
        CallableParameter::by_index(usize::MAX).unwrap()
    );
    callable.close().await.unwrap();
    assert!(callable.is_closed().unwrap());
    assert!(matches!(
        callable.out_parameter(&first),
        Err(RdbcError::ConnectionDiscarded)
    ));
}
