//! Druid raw connection 创建阶段信息。

use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 对应 Java：`DruidAbstractDataSource.PhysicalConnectionInfo`。
#[allow(dead_code)]
pub(crate) struct PhysicalConnectionInfo {
    connection: Option<Box<dyn druid_rdbc::connection::Connection>>,
    physical_connection: Option<Box<dyn druid_rdbc::spi::PhysicalConnection>>,
    connect_started_at: Instant,
    connected_at: Instant,
    initialized_at: Instant,
    validated_at: Instant,
    variables: Option<HashMap<String, Value>>,
    global_variables: Option<HashMap<String, Value>>,
    create_task_id: u64,
}

#[allow(dead_code)]
impl PhysicalConnectionInfo {
    pub(crate) fn connected(
        connection: Box<dyn druid_rdbc::connection::Connection>,
        connect_started_at: Instant,
    ) -> Self {
        let connected_at = Instant::now();
        Self {
            connection: Some(connection),
            physical_connection: None,
            connect_started_at,
            connected_at,
            initialized_at: connected_at,
            validated_at: connected_at,
            variables: None,
            global_variables: None,
            create_task_id: 0,
        }
    }

    /// 从 Task 6 transitional Physical factory result 迁入 Druid 私有阶段对象。
    pub(crate) fn from_compat(
        mut compatibility: druid_rdbc::spi::PhysicalConnectionInfo,
    ) -> Result<Self, druid_rdbc::error::RdbcError> {
        let now = Instant::now();
        let connect_span = compatibility.connect_span();
        let connect_started_at = now.checked_sub(connect_span).unwrap_or(now);
        let physical_connection = compatibility
            .take_physical_connection()
            .ok_or(druid_rdbc::error::RdbcError::ConnectionDiscarded)?;
        Ok(Self {
            connection: None,
            physical_connection: Some(physical_connection),
            connect_started_at,
            connected_at: now,
            initialized_at: now,
            validated_at: now,
            variables: compatibility.take_variables(),
            global_variables: compatibility.take_global_variables(),
            create_task_id: compatibility.create_task_id(),
        })
    }

    pub(crate) fn connection_mut(
        &mut self,
    ) -> Option<&mut Box<dyn druid_rdbc::connection::Connection>> {
        self.connection.as_mut()
    }

    pub(crate) fn take_connection(
        &mut self,
    ) -> Option<Box<dyn druid_rdbc::connection::Connection>> {
        self.connection.take()
    }

    pub(crate) fn physical_connection_box_mut(
        &mut self,
    ) -> Option<&mut Box<dyn druid_rdbc::spi::PhysicalConnection>> {
        self.physical_connection.as_mut()
    }

    pub(crate) fn take_physical_connection(
        &mut self,
    ) -> Option<Box<dyn druid_rdbc::spi::PhysicalConnection>> {
        self.physical_connection.take()
    }

    pub(crate) fn mark_initialized(&mut self) {
        self.initialized_at = Instant::now();
        self.validated_at = self.validated_at.max(self.initialized_at);
    }

    pub(crate) fn mark_validated(&mut self) {
        self.validated_at = Instant::now();
    }

    pub(crate) fn connect_span(&self) -> Duration {
        self.connected_at
            .saturating_duration_since(self.connect_started_at)
    }

    pub(crate) fn total_span(&self) -> Duration {
        self.validated_at
            .saturating_duration_since(self.connect_started_at)
    }

    pub(crate) fn set_variables(&mut self, variables: Option<HashMap<String, Value>>) {
        self.variables = variables;
    }

    pub(crate) fn take_variables(&mut self) -> Option<HashMap<String, Value>> {
        self.variables.take()
    }

    pub(crate) fn set_global_variables(
        &mut self,
        global_variables: Option<HashMap<String, Value>>,
    ) {
        self.global_variables = global_variables;
    }

    pub(crate) fn take_global_variables(&mut self) -> Option<HashMap<String, Value>> {
        self.global_variables.take()
    }

    pub(crate) fn set_create_task_id(&mut self, create_task_id: u64) {
        self.create_task_id = create_task_id;
    }
}
