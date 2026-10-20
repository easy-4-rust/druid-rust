//! 仅镜像冻结 MockResultSet 测试实际使用的原生驱动行为，不作为产品适配器能力证据。
use crate::standard_test_support::{TrackingFactory, TrackingState};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetNavigation};
use druid_rdbc::statement::{PreparedParameter, PreparedStatement, Statement, StatementOptions};
use druid_rdbc::types::{RdbcObject, RdbcTargetType, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub struct MockFactory(pub Arc<TrackingState>, pub Arc<AtomicUsize>);
#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let inner = TrackingFactory::new(Arc::clone(&self.0)).create().await?;
        Ok(Box::new(MockConnection {
            inner,
            raw_identity: Arc::clone(&self.1),
        }))
    }
}

struct MockConnection {
    inner: Box<dyn Connection>,
    raw_identity: Arc<AtomicUsize>,
}
druid_rdbc::impl_standard_wrapper!(MockConnection => Connection);
#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        assert_eq!(sql, "select ?");
        Ok(Box::new(MockPrepared {
            sql: sql.to_owned(),
            parameter: None,
            closed: false,
            raw_identity: Arc::clone(&self.raw_identity),
        }))
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.inner.auto_commit()
    }
    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.inner.set_auto_commit(enabled).await
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        self.inner.read_only()
    }
    async fn set_read_only(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.inner.set_read_only(enabled).await
    }
    async fn is_valid(&mut self, timeout: u32) -> Result<bool, RdbcError> {
        self.inner.is_valid(timeout).await
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.inner.is_closed()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.inner.close().await
    }
}

struct MockPrepared {
    sql: String,
    parameter: Option<Value>,
    closed: bool,
    raw_identity: Arc<AtomicUsize>,
}
druid_rdbc::impl_standard_wrapper!(MockPrepared => PreparedStatement);
#[async_trait::async_trait]
impl Statement for MockPrepared {
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
}
#[async_trait::async_trait]
impl PreparedStatement for MockPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        assert_eq!(
            parameter.index,
            druid_rdbc::statement::ParameterIndex::new(1).unwrap()
        );
        self.parameter = Some(parameter.value);
        Ok(())
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.parameter = None;
        Ok(())
    }
    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        assert!(!self.closed);
        assert!(matches!(self.parameter.as_ref(), Some(Value::String(_))));
        let cursor = Box::new(MockCursor::new());
        self.raw_identity.store(
            std::ptr::from_ref(cursor.as_ref()) as usize,
            Ordering::Release,
        );
        Ok(cursor)
    }
}

/// 对应 Java: MockResultSet 的 next/previous；越界 false 不改变 rowIndex。
pub struct MockCursor {
    row_index: i32,
    closed: bool,
    metadata: Arc<dyn ResultSetMetaData>,
}
impl MockCursor {
    pub fn new() -> Self {
        let metadata = BufferedRowSet::with_column_labels(
            vec![druid_rdbc::types::Row::new(vec![Value::Int(1)])],
            vec!["1".to_owned()],
        )
        .meta_data()
        .unwrap();
        Self {
            row_index: -1,
            closed: false,
            metadata,
        }
    }
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed {
            Err(RdbcError::SqlException(Box::new(SqlException::new(
                0, None, None,
            ))))
        } else {
            Ok(())
        }
    }
    fn unsupported_typed() -> RdbcError {
        // 对应 Java Mock 的 ResultSetBase#getObject(..., Class)，保留空 SQLState/消息。
        RdbcError::SqlException(Box::new(
            SqlException::new(0, None, None)
                .with_class_name("java.sql.SQLFeatureNotSupportedException"),
        ))
    }
}
druid_rdbc::impl_standard_wrapper!(MockCursor => ResultSet);
#[async_trait::async_trait]
impl ResultSet for MockCursor {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        if self.row_index < 0 {
            self.row_index += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        match navigation {
            ResultSetNavigation::Next => self.next().await,
            ResultSetNavigation::Previous => {
                if self.row_index >= 0 {
                    self.row_index -= 1;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Err(RdbcError::feature_not_supported(
                "mock_navigation_outside_frozen_cases",
            )),
        }
    }
    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Ok(self.row_index + 1)
    }
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(false)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        Ok(Arc::clone(&self.metadata))
    }
    fn object_as(
        &self,
        _column: ColumnIndex,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        assert_eq!(target, &RdbcTargetType::String);
        Err(Self::unsupported_typed())
    }
    fn object_as_by_label(
        &self,
        _label: &str,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        assert_eq!(target, &RdbcTargetType::String);
        Err(Self::unsupported_typed())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
