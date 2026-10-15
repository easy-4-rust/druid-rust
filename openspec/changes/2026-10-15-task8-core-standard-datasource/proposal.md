# Proposal

## Why

`DruidDataSource` owns the native pool and already implements the canonical `CommonDataSource` contract, but an application holding this core object cannot use the canonical `druid-rdbc::DataSource` acquisition and `Wrapper` contracts. The Task 8 logical `javax.sql.DataSource` row therefore remains planned even though the pool has the required connection lifecycle.

## What Changes

- Make the existing `DruidDataSource` implement canonical `druid-rdbc::DataSource` and `Wrapper`, returning its actual pooled logical connection without another pool or adapter object.
- Preserve the product-specific `druid::sql::DataSource` API and the wrapper's credential-aware entry. The core standard entry uses the standard feature-unsupported result for per-call credentials it cannot verify.
- Add external-consumer tests for acquisition, close/recycle, canonical unwrap, and explicit unsupported credentials; record evidence without promoting unrelated Task 8 rows.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `rdbc-common-data-source`: the native Druid data source must itself be consumable through the canonical `DataSource` and `Wrapper` contracts.

## Impact

`crates/druid/src/pool/druid_data_source.rs` gains additive trait implementations. The standard crate and wrapper APIs are unchanged. An external test exercises the new core contract. This change does not by itself close the full JDBC semantic matrix or production release gate.
