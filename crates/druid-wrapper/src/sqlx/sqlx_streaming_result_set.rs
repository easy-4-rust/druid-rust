//! Canonical eager SQLx ResultSet actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{Row, Value};
use std::sync::{Arc, Mutex};

struct ResultSetState {
    closed: bool,
    cursor: Option<usize>,
    was_null: bool,
}

pub struct SqlxStreamingResultSet {
    rows: Vec<Row>,
    meta_data: ResultSetMetaDataSnapshot,
    state: Mutex<ResultSetState>,
    actor: Option<Arc<SqlxConnectionActor>>,
}

impl SqlxStreamingResultSet {
    pub(crate) fn values(rows: Vec<Row>, labels: Vec<String>) -> Self {
        let width = rows
            .first()
            .map_or(labels.len(), Row::len)
            .max(labels.len());
        let columns = (0..width)
            .map(|index| {
                ColumnDescriptor::new(
                    labels.get(index).cloned().unwrap_or_default(),
                    "UNKNOWN",
                    ColumnNullability::Unknown,
                )
            })
            .collect();
        Self {
            rows,
            meta_data: ResultSetMetaDataSnapshot::new(columns),
            state: Mutex::new(ResultSetState {
                closed: false,
                cursor: None,
                was_null: false,
            }),
            actor: None,
        }
    }

    pub(crate) fn with_actor_lease(mut self, actor: Arc<SqlxConnectionActor>) -> Self {
        self.actor = Some(actor);
        self
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.actor.as_ref().is_some_and(|actor| actor.is_closed()) || self.state().closed {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ResultSetState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl std::fmt::Debug for SqlxStreamingResultSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqlxStreamingResultSet")
            .field("row_count", &self.rows.len())
            .field("owns_actor", &self.actor.is_some())
            .finish_non_exhaustive()
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxStreamingResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for SqlxStreamingResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let mut state = self.state();
        let next = state.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        let present = next < self.rows.len();
        state.cursor = present.then_some(next);
        state.was_null = false;
        drop(state);
        if present {
            if let Some(actor) = &self.actor {
                actor.task4_trace_result_next();
            }
        }
        Ok(present)
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        let offset = column.get() - 1;
        let mut state = self.state();
        let row = state
            .cursor
            .and_then(|cursor| self.rows.get(cursor))
            .ok_or_else(|| RdbcError::InvalidArgument("result set is not on a row".to_owned()))?;
        let value = row.get(offset).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!("column {} is out of range", column.get()))
        })?;
        state.was_null = matches!(value, Value::Null);
        Ok(value)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        if let Some(actor) = &self.actor {
            actor.task4_trace_result_set_metadata();
        }
        Ok(Arc::new(self.meta_data.clone()))
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(self.state().was_null)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state().closed = true;
        self.actor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.state().closed)
    }
}
