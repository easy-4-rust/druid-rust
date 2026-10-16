# Design

The current Druid implementation is the behavior baseline. Its `RegisteredDriver` registry and process-wide settings move into a single standard-layer object file, while the historical Druid module contains only a named re-export. `DriverRegistry` remains the pure per-attempt negotiation algorithm; the manager snapshots registered drivers before invoking third-party callbacks and applies one overall login timeout. The standard layer already owns `Driver`, `DriverAction`, `ConnectionProperties`, `SqlException`, and `RdbcLogWriter`.

Test type and state identity from outside both crates before migration. Re-run callback/reentrancy, snapshot, timeout, diagnostic redaction, and public API checks. Advance the semantic inventory row only after the standard implementation has current behavioral evidence; do not mark unrelated `Driver`, adapter, or production gates complete.

The approved semantic correction maps `java.sql.DriverManager` to `druid_rdbc::driver::DriverManager`; `DriverRegistry` stays the internal per-attempt algorithm. Update the frozen mapping digest while retaining every OpenJDK source record and its source-manifest hash. The manager's own cross-path runtime test permits `V1_LIVE` capability evidence, but does not advance unrelated inventory rows.
