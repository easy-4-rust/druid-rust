//! Canonical eager SQLx ResultSet actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ColumnDescriptor, ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetNavigation};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};
use druid_rdbc::types::{Row, Value};
use std::sync::{Arc, Mutex};

struct ResultSetState {
    closed: bool,
    cursor: i64,
    was_null: bool,
    fetch_direction: FetchDirection,
    fetch_size: i32,
}

pub struct SqlxStreamingResultSet {
    rows: Vec<Row>,
    meta_data: ResultSetMetaDataSnapshot,
    state: Mutex<ResultSetState>,
    actor: Option<Arc<SqlxConnectionActor>>,
}

impl SqlxStreamingResultSet {
    pub(crate) fn values(rows: Vec<Row>, columns: Vec<ColumnDescriptor>) -> Self {
        Self {
            rows,
            meta_data: ResultSetMetaDataSnapshot::new(columns),
            state: Mutex::new(ResultSetState {
                closed: false,
                cursor: -1,
                was_null: false,
                fetch_direction: FetchDirection::Unknown,
                fetch_size: 0,
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
        let present = self.navigate(ResultSetNavigation::Next).await?;
        if present {
            if let Some(actor) = &self.actor {
                actor.task4_trace_result_next();
            }
        }
        Ok(present)
    }

    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let len = i64::try_from(self.rows.len()).unwrap_or(i64::MAX);
        let mut state = self.state();
        let cursor = match navigation {
            ResultSetNavigation::Next => state.cursor.saturating_add(1),
            ResultSetNavigation::Previous if state.cursor >= len => len.saturating_sub(1),
            ResultSetNavigation::Previous => state.cursor.saturating_sub(1),
            ResultSetNavigation::First => 0,
            ResultSetNavigation::Last => len.saturating_sub(1),
            ResultSetNavigation::BeforeFirst | ResultSetNavigation::Absolute(0) => -1,
            ResultSetNavigation::AfterLast => len,
            ResultSetNavigation::Absolute(row) if row > 0 => i64::from(row) - 1,
            ResultSetNavigation::Absolute(row) => len.saturating_add(i64::from(row)),
            ResultSetNavigation::Relative(rows) => state.cursor.saturating_add(i64::from(rows)),
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "sqlx_result_set_navigation",
                ));
            }
        };
        let present = cursor >= 0 && cursor < len;
        state.cursor = if present {
            cursor
        } else if cursor < 0 {
            -1
        } else {
            len
        };
        state.was_null = false;
        Ok(present)
    }

    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        let state = self.state();
        if state.cursor >= 0
            && usize::try_from(state.cursor).is_ok_and(|cursor| cursor < self.rows.len())
        {
            i32::try_from(state.cursor + 1)
                .map_err(|_| RdbcError::InvalidArgument("row exceeds JDBC i32".to_owned()))
        } else {
            Ok(0)
        }
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        let offset = column.get() - 1;
        let mut state = self.state();
        let row = usize::try_from(state.cursor)
            .ok()
            .and_then(|cursor| self.rows.get(cursor))
            .ok_or_else(|| RdbcError::InvalidArgument("result set is not on a row".to_owned()))?;
        let value = row.get(offset).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!("column {} is out of range", column.get()))
        })?;
        state.was_null = std::matches!(value, Value::Null);
        Ok(value)
    }

    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.ensure_open()?;
        self.meta_data
            .columns()
            .iter()
            .position(|column| column.label().eq_ignore_ascii_case(column_label))
            .and_then(|offset| ColumnIndex::new(offset + 1).ok())
            .ok_or_else(|| RdbcError::InvalidArgument(format!("unknown column: {column_label}")))
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

    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let state = self.state();
        Ok(!self.rows.is_empty() && state.cursor < 0)
    }

    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let state = self.state();
        Ok(!self.rows.is_empty()
            && usize::try_from(state.cursor).is_ok_and(|cursor| cursor >= self.rows.len()))
    }

    fn is_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(!self.rows.is_empty() && self.state().cursor == 0)
    }

    fn is_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let last = i64::try_from(self.rows.len()).unwrap_or(i64::MAX) - 1;
        Ok(!self.rows.is_empty() && self.state().cursor == last)
    }

    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state().fetch_direction = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().fetch_direction)
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        if rows < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must not be negative".to_owned(),
            ));
        }
        self.state().fetch_size = rows;
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Ok(self.state().fetch_size)
    }

    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        Ok(ResultSetType::ScrollInsensitive)
    }

    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        Ok(ResultSetConcurrency::ReadOnly)
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(ResultSetHoldability::HoldOverCommit)
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()
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
