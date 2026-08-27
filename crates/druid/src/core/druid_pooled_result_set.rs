//! Canonical pooled ResultSet proxy.

mod access;

use super::druid_pooled_statement::{
    DruidPooledStatement, DruidPooledStatementInner, TrackedStatementResult,
};
use super::result_set_meta_data_proxy::ResultSetMetaDataProxy;
use super::standard_resource_admission::StandardResourceAdmission;
use super::standard_resource_handle::StandardResourceHandle;
use super::standard_resource_registry::StandardResourceRegistry;
use super::{FilterChain, ProxyAttributeValue, ProxyAttributes, RdbcError, ResultSetFilterContext};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{ExecutionResult, Statement};
use druid_rdbc::types::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct StandardResultSetResource {
    result_set: tokio::sync::Mutex<Option<Box<dyn ResultSet>>>,
    opened: AtomicBool,
    closing: AtomicBool,
    closed: AtomicBool,
    close_error: Mutex<Option<RdbcError>>,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
    policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
    meta_data_attributes: Arc<ProxyAttributes>,
    registration: OnceLock<super::standard_resource_registration::StandardResourceRegistration>,
}

impl StandardResultSetResource {
    fn new(
        result_set: Box<dyn ResultSet>,
        filter_chain: Option<Arc<FilterChain>>,
        filter_context: Arc<ResultSetFilterContext>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
    ) -> Arc<Self> {
        Arc::new(Self {
            result_set: tokio::sync::Mutex::new(Some(result_set)),
            opened: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            close_error: Mutex::new(None),
            filter_chain,
            filter_context,
            policy,
            meta_data_attributes: Arc::new(ProxyAttributes::default()),
            registration: OnceLock::new(),
        })
    }

    async fn next_core(&self) -> Result<bool, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let mut result_set = self.result_set.lock().await;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_next(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            None => raw.next().await,
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    pub(crate) async fn close_core(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            self.unregister_resource();
            return Ok(());
        }
        self.closing.store(true, Ordering::Release);
        let mut result_set = self.result_set.lock().await;
        let Some(raw) = result_set.as_mut() else {
            self.closed.store(true, Ordering::Release);
            self.unregister_resource();
            return Ok(());
        };
        let opened = self.opened.load(Ordering::Acquire);
        let result = match (&self.filter_chain, opened) {
            (Some(filter_chain), true) => filter_chain
                .standard_result_set_close(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            (Some(_), false) | (None, _) => raw.close().await,
        };
        if let Err(error) = self.policy.classify(result, self.filter_context.sql()) {
            *self
                .close_error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error.clone());
            return Err(error);
        }
        self.close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        result_set.take();
        self.closed.store(true, Ordering::Release);
        if opened {
            self.filter_context.increment_close_count();
        }
        self.unregister_resource();
        Ok(())
    }

    fn registry_cleanup_error(&self) -> Option<RdbcError> {
        if let Some(error) = self
            .close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            return Some(error);
        }
        let cleanup_requested = self.registration.get().is_some_and(
            super::standard_resource_registration::StandardResourceRegistration::cleanup_requested,
        );
        (cleanup_requested
            || self.closing.load(Ordering::Acquire)
            || self.closed.load(Ordering::Acquire))
        .then_some(RdbcError::ConnectionDiscarded)
    }

    fn open_meta_data_core(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        let result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let meta_data = self
            .policy
            .classify(raw.meta_data(), self.filter_context.sql())?;
        Ok(Arc::new(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        )))
    }

    fn value_core(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_value(raw.as_mut(), &self.filter_context, column)
                .map_err(RdbcError::from),
            None => raw.value(column),
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    fn meta_data_core(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_meta_data(raw.as_mut(), &self.filter_context)
                .map_err(RdbcError::from),
            None => raw.meta_data(),
        };
        let meta_data = self.policy.classify(result, self.filter_context.sql())?;
        Ok(Arc::new(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        )))
    }

    fn was_null_core(&self) -> Result<bool, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_was_null(raw.as_mut(), &self.filter_context)
                .map_err(RdbcError::from),
            None => raw.was_null(),
        };
        self.policy.classify(result, self.filter_context.sql())
    }
}

#[async_trait::async_trait]
impl StandardResourceHandle for StandardResultSetResource {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        self.close_core().await
    }

    fn is_resource_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn registration_slot(
        &self,
    ) -> &OnceLock<super::standard_resource_registration::StandardResourceRegistration> {
        &self.registration
    }
}

/// Pooled canonical cursor bound to one logical connection generation.
pub struct DruidPooledResultSet {
    id: u64,
    attributes: ProxyAttributes,
    statement: DruidPooledStatement,
    resource: Arc<StandardResultSetResource>,
    closed: AtomicBool,
    cursor_index: AtomicI32,
    filter_context: Arc<ResultSetFilterContext>,
    logic_column_map: Option<HashMap<i32, i32>>,
    driver_column_map: Option<HashMap<i32, i32>>,
    hidden_columns: Option<Vec<i32>>,
}

struct StagedResultSet {
    id: u64,
    statement: DruidPooledStatement,
    resource: Arc<StandardResultSetResource>,
    filter_chain: Option<Arc<FilterChain>>,
    filter_context: Arc<ResultSetFilterContext>,
}

struct StagedOpenError {
    error: RdbcError,
    cleanup_required: bool,
}

impl StagedResultSet {
    fn new(statement: Arc<DruidPooledStatementInner>, raw: Box<dyn ResultSet>) -> Self {
        let statement_handle = DruidPooledStatement::from_inner(Arc::clone(&statement));
        let id = statement.result_set_id_seed.fetch_add(1, Ordering::AcqRel);
        let filter_context = Arc::new(
            ResultSetFilterContext::with_identity_sql_and_execute_elapsed(
                statement.connection_id,
                statement.id,
                id,
                statement_handle.last_sql(),
                statement_handle.last_execute_elapsed(),
            ),
        );
        let filter_chain = statement.filter_chain.clone();
        let resource = StandardResultSetResource::new(
            raw,
            filter_chain.clone(),
            Arc::clone(&filter_context),
            Arc::clone(&statement.policy),
        );
        statement.track_result_set(&resource);
        Self {
            id,
            statement: statement_handle,
            resource,
            filter_chain,
            filter_context,
        }
    }

    fn into_opened(self) -> Result<DruidPooledResultSet, StagedOpenError> {
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        let meta_data = self
            .resource
            .open_meta_data_core()
            .map_err(|error| StagedOpenError {
                error,
                cleanup_required: true,
            })?;
        let mut context = super::StandardResultSetOpenContext::new(&self.filter_context, meta_data);
        if let Some(filter_chain) = &self.filter_chain {
            self.resource
                .policy
                .classify(
                    filter_chain
                        .standard_result_set_open_after_with_proxy(&mut context)
                        .map_err(RdbcError::from),
                    self.filter_context.sql(),
                )
                .map_err(|error| StagedOpenError {
                    error,
                    cleanup_required: true,
                })?;
        }
        if let Some(error) = self.resource.registry_cleanup_error() {
            return Err(StagedOpenError {
                error,
                cleanup_required: false,
            });
        }
        let (driver_column_map, logic_column_map, hidden_columns) = context.into_column_mappings();
        self.resource.opened.store(true, Ordering::Release);
        Ok(DruidPooledResultSet {
            id: self.id,
            attributes: ProxyAttributes::default(),
            statement: self.statement,
            resource: self.resource,
            closed: AtomicBool::new(false),
            cursor_index: AtomicI32::new(0),
            filter_context: self.filter_context,
            logic_column_map,
            driver_column_map,
            hidden_columns,
        })
    }
}

enum StagedExecutionResult {
    ResultSet(StagedResultSet),
    UpdateCount(i64),
}

impl DruidPooledResultSet {
    pub(crate) async fn new_standard(
        statement: Arc<DruidPooledStatementInner>,
        raw: Box<dyn ResultSet>,
        admission: &StandardResourceAdmission,
    ) -> Result<Self, RdbcError> {
        let resources = Arc::clone(&statement.resources);
        let statement_identity = Arc::clone(&statement);
        let staged = StagedResultSet::new(statement, raw);
        let handle = Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>;
        let registrations = resources.register_batch(admission, vec![handle]).await?;
        match staged.into_opened() {
            Ok(result_set) => {
                statement_identity.set_execution_results(vec![TrackedStatementResult::ResultSet(
                    Arc::downgrade(&result_set.resource),
                )]);
                Ok(result_set)
            }
            Err(open_error) => {
                let cleanup = resources
                    .close_registered_after_admission(&registrations, admission)
                    .await;
                if open_error.cleanup_required {
                    Err(open_error.error)
                } else {
                    cleanup.and(Err(open_error.error))
                }
            }
        }
    }

    pub(crate) async fn new_standard_results(
        statement: Arc<DruidPooledStatementInner>,
        results: Vec<ExecutionResult>,
        admission: &StandardResourceAdmission,
    ) -> Result<Vec<ExecutionResult>, RdbcError> {
        let resources = Arc::clone(&statement.resources);
        let mut staged_results = Vec::with_capacity(results.len());
        let mut handles = Vec::new();
        for result in results {
            match result {
                ExecutionResult::ResultSet(raw) => {
                    let staged = StagedResultSet::new(Arc::clone(&statement), raw);
                    handles.push(Arc::clone(&staged.resource) as Arc<dyn StandardResourceHandle>);
                    staged_results.push(StagedExecutionResult::ResultSet(staged));
                }
                ExecutionResult::UpdateCount(count) => {
                    staged_results.push(StagedExecutionResult::UpdateCount(count));
                }
            }
        }
        let registrations = resources.register_batch(admission, handles).await?;
        let mut opened = Vec::with_capacity(staged_results.len());
        let mut tracked = Vec::with_capacity(staged_results.len());
        for staged in staged_results {
            match staged {
                StagedExecutionResult::ResultSet(staged) => match staged.into_opened() {
                    Ok(result_set) => {
                        tracked.push(TrackedStatementResult::ResultSet(Arc::downgrade(
                            &result_set.resource,
                        )));
                        opened.push(ExecutionResult::ResultSet(Box::new(result_set)));
                    }
                    Err(open_error) => {
                        let cleanup = resources
                            .close_registered_after_admission(&registrations, admission)
                            .await;
                        return if open_error.cleanup_required {
                            Err(open_error.error)
                        } else {
                            cleanup.and(Err(open_error.error))
                        };
                    }
                },
                StagedExecutionResult::UpdateCount(count) => {
                    tracked.push(TrackedStatementResult::UpdateCount(count));
                    opened.push(ExecutionResult::UpdateCount(count));
                }
            }
        }
        statement.set_execution_results(tracked);
        Ok(opened)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new_standard_metadata(
        connection_id: u64,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        lease_active: Arc<AtomicBool>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<crate::stats::StatsCollector>>,
        resources: Arc<StandardResourceRegistry>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        admission: &StandardResourceAdmission,
        raw: Box<dyn ResultSet>,
    ) -> Result<Self, RdbcError> {
        let statement = DruidPooledStatement::new_standard(
            Box::new(MetadataStatement),
            druid_rdbc::statement::StatementOptions::default(),
            resources,
            policy,
            connection_id,
            0,
            result_set_id_seed,
            Arc::new(AtomicU64::new(0)),
            lease_active,
            filter_chain,
            stats_collector,
        );
        Self::new_standard(statement.inner, raw, admission).await
    }

    /// Returns the result-set identifier.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn statement(&self) -> DruidPooledStatement {
        self.statement.clone()
    }

    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.attributes.len()
    }

    pub fn clear_attributes(&self) {
        self.attributes.clear();
    }

    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.attributes.put(key, value)
    }

    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.attributes.get(key)
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire)
            || !self.statement.inner.lease_active.load(Ordering::Acquire)
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn driver_column(&self, logical: i32) -> Option<i32> {
        let driver = self
            .driver_column_map
            .as_ref()
            .and_then(|mapping| mapping.get(&logical).copied())
            .unwrap_or(logical);
        if self
            .hidden_columns
            .as_ref()
            .is_some_and(|columns| columns.contains(&driver))
        {
            return None;
        }
        Some(driver)
    }

    fn driver_column_index(&self, column: ColumnIndex) -> Result<ColumnIndex, RdbcError> {
        let logical = i32::try_from(column.get())
            .map_err(|_| RdbcError::InvalidArgument("column index exceeds i32".to_owned()))?;
        let driver = self
            .driver_column(logical)
            .ok_or_else(|| RdbcError::InvalidArgument("column is hidden".to_owned()))?;
        usize::try_from(driver)
            .ok()
            .and_then(|index| ColumnIndex::new(index).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid driver column".to_owned()))
    }

    fn logical_column_index(&self, driver: ColumnIndex) -> Result<ColumnIndex, RdbcError> {
        let driver = i32::try_from(driver.get())
            .map_err(|_| RdbcError::InvalidArgument("column index exceeds i32".to_owned()))?;
        if self
            .hidden_columns
            .as_ref()
            .is_some_and(|columns| columns.contains(&driver))
        {
            return Err(RdbcError::InvalidArgument("column is hidden".to_owned()));
        }
        let logical = self
            .logic_column_map
            .as_ref()
            .and_then(|mapping| mapping.get(&driver).copied())
            .unwrap_or(driver);
        usize::try_from(logical)
            .ok()
            .and_then(|index| ColumnIndex::new(index).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid logical column".to_owned()))
    }
}

#[derive(Debug)]
struct MetadataStatement;

druid_rdbc::impl_standard_wrapper!(MetadataStatement => Statement);

#[async_trait::async_trait]
impl Statement for MetadataStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

druid_rdbc::impl_standard_wrapper!(DruidPooledResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for DruidPooledResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        let more = self.resource.next_core().await?;
        if more {
            let cursor = self.cursor_index.fetch_add(1, Ordering::AcqRel) + 1;
            self.filter_context.record_fetch_row_count(cursor);
        }
        Ok(more)
    }

    async fn navigate(
        &mut self,
        navigation: druid_rdbc::result_set::ResultSetNavigation,
    ) -> Result<bool, RdbcError> {
        if std::matches!(
            navigation,
            druid_rdbc::result_set::ResultSetNavigation::Next
        ) {
            return self.next().await;
        }
        self.ensure_open()?;
        let present = self.resource.navigate_core(navigation).await?;
        let row = self.resource.access_core(|raw| raw.row())?;
        self.cursor_index.store(row, Ordering::Release);
        if present {
            self.filter_context.record_fetch_row_count(row);
        }
        Ok(present)
    }

    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.row())
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        self.resource.value_core(self.driver_column_index(column)?)
    }

    fn find_column(&self, column_label: &str) -> Result<ColumnIndex, RdbcError> {
        self.ensure_open()?;
        let driver = self
            .resource
            .access_core(|raw| raw.find_column(column_label))?;
        self.logical_column_index(driver)
    }

    fn object(&self, column: ColumnIndex) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        self.resource
            .access_core(|raw| raw.object(self.driver_column_index(column)?))
    }

    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        type_map: Option<&druid_rdbc::types::RdbcTypeMap>,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| {
            raw.object_with_type_map(self.driver_column_index(column)?, type_map)
        })
    }

    fn object_as(
        &self,
        column: ColumnIndex,
        target_type: &druid_rdbc::types::RdbcTargetType,
    ) -> Result<druid_rdbc::types::RdbcObject, RdbcError> {
        self.ensure_open()?;
        self.resource
            .access_core(|raw| raw.object_as(self.driver_column_index(column)?, target_type))
    }

    fn input_stream(
        &self,
        column: ColumnIndex,
        kind: druid_rdbc::result_set::ResultSetStreamKind,
    ) -> Result<Option<druid_rdbc::resource::RdbcInputStream>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .access_core(|raw| raw.input_stream(self.driver_column_index(column)?, kind))
    }

    fn reader(
        &self,
        column: ColumnIndex,
        national: bool,
    ) -> Result<Option<druid_rdbc::resource::RdbcReader>, RdbcError> {
        self.ensure_open()?;
        self.resource
            .access_core(|raw| raw.reader(self.driver_column_index(column)?, national))
    }

    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: druid_rdbc::result_set::ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let result = self
            .resource
            .access_core(|raw| raw.update_value(self.driver_column_index(column)?, update));
        if let Err(error) = &result {
            self.resource.policy.record_result_set_error(error);
        }
        result
    }

    fn meta_data(&self) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        self.resource.meta_data_core()
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.was_null_core()
    }

    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.is_before_first())
    }

    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.is_after_last())
    }

    fn is_first(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.is_first())
    }

    fn is_last(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.is_last())
    }

    fn set_fetch_direction(
        &mut self,
        direction: druid_rdbc::statement::FetchDirection,
    ) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource
            .access_core(|raw| raw.set_fetch_direction(direction))
    }

    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.fetch_direction())
    }

    fn set_fetch_size(&mut self, rows: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.set_fetch_size(rows))
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.fetch_size())
    }

    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.result_set_type())
    }

    fn concurrency(&self) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.concurrency())
    }

    fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.holdability())
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.warnings())
    }

    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::clear_warnings)
    }

    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.cursor_name())
    }

    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.row_updated())
    }

    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.row_inserted())
    }

    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(|raw| raw.row_deleted())
    }

    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::insert_row)
    }

    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::update_row)
    }

    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::delete_row)
    }

    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::refresh_row)
    }

    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::cancel_row_updates)
    }

    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::move_to_insert_row)
    }

    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.resource.access_core(ResultSet::move_to_current_row)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.resource.close_resource().await?;
        self.closed.store(true, Ordering::Release);
        self.statement
            .record_fetch_row_count(self.filter_context.fetch_row_count());
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire) || self.resource.is_resource_closed())
    }
}

impl std::fmt::Debug for DruidPooledResultSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledResultSet")
            .field("id", &self.id)
            .field("closed", &self.closed.load(Ordering::Acquire))
            .field("cursor", &self.cursor_index.load(Ordering::Acquire))
            .field("logic_column_map", &self.logic_column_map)
            .finish_non_exhaustive()
    }
}
