//! Druid driver connection acquisition timing and session metadata.

use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Private acquisition envelope consumed by [`super::DruidConnectionHolder`].
pub(crate) struct ConnectionInfo {
    connection: Option<Box<dyn druid_rdbc::connection::Connection>>,
    connect_started_at: Instant,
    connected_at: Instant,
    initialized_at: Instant,
    validated_at: Instant,
    variables: Option<HashMap<String, Value>>,
    global_variables: Option<HashMap<String, Value>>,
}

impl ConnectionInfo {
    pub(crate) fn connected(
        connection: Box<dyn druid_rdbc::connection::Connection>,
        connect_started_at: Instant,
    ) -> Self {
        let connected_at = Instant::now();
        Self {
            connection: Some(connection),
            connect_started_at,
            connected_at,
            initialized_at: connected_at,
            validated_at: connected_at,
            variables: None,
            global_variables: None,
        }
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
}
