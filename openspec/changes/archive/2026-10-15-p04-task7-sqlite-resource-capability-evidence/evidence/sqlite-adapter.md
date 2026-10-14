# SQLx SQLite resource and asynchronous result-set evidence

## Runtime boundary

This test connects `SqlxConnectionAdapter` to an actual SQLx in-memory SQLite connection (`sqlite::memory:`). The adapter reports transactions and savepoints as supported, with catalogs, schemas, and callable statements unsupported. Its standard Array, Blob, Clob, NClob, SQLXML, and Struct factory methods return SQLState `0A000`. That is the adapter's actual behavior; it is not a claim that SQLite or any other SQLx backend implements those resource factories.

The same connection executes `SELECT 10 UNION ALL SELECT 20 UNION ALL SELECT 30` through the standard `Statement` and asynchronous `ResultSet` traits. The test reads all three integer rows in order, observes EOF, and explicitly closes the result set, statement, and connection.

## Verification

Environment: Windows x86_64; `cargo 1.97.1 (c980f4866 2026-06-30)`; `rustc 1.97.1 (8bab26f4f 2026-07-14)`; SQLx `0.8.6` from `Cargo.lock`.

| Command | Exit | Result |
|---|---:|---|
| `cargo fmt -p druid-wrapper -- --check` | 0 | Formatting clean |
| `cargo test -p druid-wrapper --features sqlx --lib sqlite_live_resources_capabilities_and_async_result_set_are_truthful --locked` | 0 | 1 passed; 20 filtered out |
| `openspec validate p04-task7-sqlite-resource-capability-evidence --strict --no-interactive` | 0 | Change valid |
| `git diff --check` | 0 | No whitespace errors |

Source SHA-256 for `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`: `2D468AAC12865C17FF2D6B66AE955AEAEBD8E1B601E265E19EE9677C36A70FE1`.

## Limits

This closes a real SQLite SQLx evidence gap only. It does not demonstrate real database behavior for Toasty, PostgreSQL, or MySQL; it does not certify every JDBC method, a JDBC TCK, or project-wide production readiness. Backend-specific resource support remains individually unverified unless covered by a separate live backend test.
