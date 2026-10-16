# JDBC Agent NClob resource conformance

## Why

The canonical `NClob` contract exists, but the JDBC Agent connection still returns the default unsupported result for `create_n_clob`. Task 8 therefore has no live adapter or driver evidence for `java.sql.NClob`.

## What Changes

- Create and own an actual driver `NClob` in the Agent session, exposing it through the canonical RDBC `NClob` entry.
- Reuse the proven bounded Clob operations where the inherited JDBC contract is identical, while preserving the resource's NClob identity and driver errors.
- Verify H2 and HSQLDB behavior separately, including unsupported driver outcomes, child streams, release, and pooled-owner close.
- Advance only the `java.sql.NClob` inventory row after positive external-consumer evidence; retain unrelated rows as planned.

## Capabilities

### Modified Capabilities

- `rdbc-jdbc-method-conformance`: Add independently verified JDBC Agent NClob resource and ownership behavior.

## Impact

Java Agent protocol/session ownership, Rust JDBC Agent adapter, real-driver tests, Task 8 inventory, API/SemVer evidence, and cross-platform CI. The change does not claim SQLXML, Ref, Struct, SQLData, or other backend NClob support.
