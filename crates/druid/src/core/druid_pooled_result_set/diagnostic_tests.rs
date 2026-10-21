//! 用真实 SQLite 验证私有结果集上下文，保留迁移前的状态合同。

use super::DruidPooledResultSet;
use crate::core::druid_pooled_statement::DruidPooledStatement;
use crate::core::owner_disposition_control::OwnerDispositionControl;
use crate::core::standard_logical_policy::StandardLogicalPolicy;
use crate::core::standard_resource_registry::StandardResourceRegistry;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::result_set::ResultSetExt;
use druid_rdbc::result_set::ResultSetUpdate;
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_rdbc::types::{
    RdbcBigDecimal as BigDecimal, RdbcCharacterLength, RdbcInputStream, RdbcNaiveDate as NaiveDate,
    RdbcNaiveTime as NaiveTime, RdbcObject, RdbcOpaqueObject, RdbcReader, RdbcStreamLength, Value,
};
use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

mod filter_defaults_chain_harness;
mod filter_defaults_result_set_probe;
mod filter_order_result_set_probe;
mod initial_column_map_filter;
mod int_wrapping_filter;
mod legacy_metadata_observer;
mod lob_stream_defaults_result_set_probe;
mod lob_stream_update_short_circuit_filter;
mod metadata_read_probe;
mod metadata_short_circuit_filter;
mod n_string_read_probe;
mod n_string_short_circuit_filter;
mod n_string_update_short_circuit_filter;
mod native_result_set_close_observer;
mod navigation_defaults_chain_harness;
mod navigation_defaults_result_set_probe;
mod navigation_short_circuit_filter;
mod object_payload;
mod object_read_probe;
mod object_short_circuit_filter;
mod object_update_short_circuit_filter;
mod object_update_vendor_probe;
mod ordered_metadata_filter;
mod ordered_n_string_update_filter;
mod ordered_navigation_property_filter;
mod ordered_object_filter;
mod ordered_resource_filter;
mod ordered_scalar_filter;
mod ordered_statement_filter;
mod ordering_result_set_filter;
mod resource_defaults_chain_harness;
mod resource_defaults_result_set_probe;
mod resource_read_probe;
mod resource_short_circuit_filter;
mod resource_update_defaults_chain_harness;
mod resource_update_defaults_result_set_probe;
mod resource_update_short_circuit_filter;
mod row_mutation_defaults_chain_harness;
mod row_mutation_defaults_result_set_probe;
mod row_mutation_short_circuit_filter;
mod scalar_getter_delegation_probe;
mod scalar_read_probe;
mod scalar_short_circuit_filter;
mod scalar_update_short_circuit_filter;
mod sparse_result_set;
mod sql_capture_filter;
mod stat_close_error_chain_harness;
mod stat_close_error_result_set_probe;
mod statement_short_circuit_filter;
mod stream_update_defaults_chain_harness;
mod stream_update_defaults_result_set_probe;
mod stream_update_short_circuit_filter;
mod strong_getter_call;
mod strong_getter_filter;
mod strong_getter_probe;
mod strong_getter_short_circuit_filter;
mod update_call;
mod update_identity_probe;
mod vendor_object_probe;

mod harness;

mod n_string_updates;

mod update_object_errors;

mod diagnostic_basics;

mod update_overloads;

mod typed_getters;

mod capabilities;

mod navigation_values;

mod filter_overloads_a;

mod filter_overloads_b;

mod scalar_filters;

mod metadata_navigation;

mod resource_routes_a;

mod resource_routes_b;

mod stream_and_stat;

mod filter_defaults_scalar;

mod filter_defaults_stream_a;

mod filter_defaults_stream_b;
