# Proposal

## Why

`java.sql.Blob` remains `PLANNED` in the JDBC 4.2 adapter inventory. The standard RDBC Blob contract and Druid's pooled resource owner exist, but the JDBC Agent cannot create or operate a driver Blob, so application code cannot use that contract through this adapter.

## What Changes

- Add session-owned JDBC Blob handles and their child streams to the Agent protocol, with bounded transfers and deterministic close order.
- Implement the standard Blob methods on the Rust JDBC Agent adapter, preserving one-based JDBC positions, driver errors, release semantics, and truthful per-operation capabilities.
- Verify the protocol and external RDBC consumer against real H2, including SQL NULL boundaries, failed release, session/pooled-owner close, and large streamed data.
- Advance only the `java.sql.Blob` adapter inventory row after its behavior is proven.

## Capabilities

### Modified Capabilities

- `rdbc-jdbc-method-conformance`: Add backend-scoped Blob behavior and evidence requirements for the JDBC Agent.

## Impact

Touches the Java Agent session/protocol, `druid-wrapper::jdbc_agent`, resource tests, and the Task 8 semantic inventory. No dependency or capability claim changes for other adapters.
