# Design

Extend `verify_capabilities` in `capability_truthfulness_live_test.rs`, which already runs SQLx and Toasty against SQLite and, in CI, PostgreSQL/MySQL. Check each default resource factory for the precise `SqlException` SQLState `0A000`. After the existing transaction/savepoint assertions, create a standard Statement and query the session-local table's committed row through `ResultSet::next` and `value`. Verify EOF, close the result set and statement, then close the connection. Keep all checks in one backend-neutral test body so a matrix entry cannot silently omit one adapter.

No production code changes are expected. A failing assertion indicates a real adapter capability or lifecycle gap and must be diagnosed rather than weakened.
