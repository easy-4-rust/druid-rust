//! Wrapper-private canonical eager ResultSet for non-SQLx adapter cores.

use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{Row, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(crate) struct StandardAdapterResultSet {
    rows: Vec<Row>,
    cursor: usize,
    positioned: bool,
    was_null: AtomicBool,
    closed: bool,
    lease: Option<Arc<dyn StandardExecutor>>,
    metadata: Arc<ResultSetMetaDataSnapshot>,
}

impl StandardAdapterResultSet {
    pub(crate) fn new(rows: Vec<Row>, lease: Arc<dyn StandardExecutor>) -> Self {
        let width = rows.first().map_or(0, |row| row.values.len());
        let columns = (1..=width)
            .map(|index| {
                ColumnDescriptor::new(
                    format!("column_{index}"),
                    "UNKNOWN",
                    ColumnNullability::Unknown,
                )
            })
            .collect();
        Self {
            rows,
            cursor: 0,
            positioned: false,
            was_null: AtomicBool::new(false),
            closed: false,
            lease: Some(lease),
            metadata: Arc::new(ResultSetMetaDataSnapshot::new(columns)),
        }
    }

    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        if self.closed {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for StandardAdapterResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        let next = if self.positioned {
            self.cursor.saturating_add(1)
        } else {
            0
        };
        self.positioned = next < self.rows.len();
        self.cursor = next;
        self.was_null.store(false, Ordering::Release);
        Ok(self.positioned)
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open_core()?;
        if !self.positioned {
            return Err(RdbcError::InvalidArgument(
                "result set has no current row".to_owned(),
            ));
        }
        let value = self
            .rows
            .get(self.cursor)
            .and_then(|row| row.values.get(column.get() - 1))
            .cloned()
            .ok_or_else(|| {
                RdbcError::InvalidArgument("column index exceeds row width".to_owned())
            })?;
        self.was_null
            .store(matches!(value, Value::Null), Ordering::Release);
        Ok(value)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.metadata.clone())
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        Ok(self.was_null.load(Ordering::Acquire))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.lease.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
