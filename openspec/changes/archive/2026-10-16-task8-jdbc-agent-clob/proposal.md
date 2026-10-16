# JDBC Agent Clob resource conformance

## Why

`java.sql.Clob` is still `PLANNED` in the Task 8 adapter inventory. The canonical RDBC contract and pooled owner exist, but the JDBC Agent cannot create a driver Clob or expose its operations through the adapter.

## What Changes

- Own driver Clob handles and child streams within the Agent session, including retryable failed release and cleanup on logical/session close.
- Expose Clob character length, UTF-16-preserving values and offsets, search, mutation, truncation, and bounded character/ASCII streams through the standard RDBC adapter.
- Preserve driver errors and advertise capabilities only after real-driver positive evidence. Test H2 and HSQLDB separately where their Clob behavior differs.
- Advance only the proven `java.sql.Clob` adapter inventory row after external-consumer and ownership tests pass.

## Impact

The change adds a JDBC Agent Clob path in Java and Rust, protocol tests, real-driver external-consumer tests, and Task 8 evidence. It does not assert NClob or other backend support.
