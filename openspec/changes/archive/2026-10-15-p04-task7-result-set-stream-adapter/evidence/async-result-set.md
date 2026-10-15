# P04 Task7 asynchronous ResultSet evidence

## Cancellation recovery

The module unit test creates a pending oneshot row response, polls `next` until it is pending, then supplies a row through that same response. The next invocation returns the row and advances the row number to 1. This validates cancellation recovery of an already accepted response without timing sleeps or a database-dependent race.

## Live SQLx SQLite boundary

The archived change [`2026-10-15-p04-task7-sqlite-resource-capability-evidence`](../../archive/2026-10-15-p04-task7-sqlite-resource-capability-evidence/evidence/sqlite-adapter.md) records a live `sqlite::memory:` test that verifies SQLx SQLite capability reporting, SQLState `0A000` for unsupported Array/Blob/Clob/NClob/SQLXML/Struct factories, ordered rows, EOF, and explicit close.

## Verification

| Command | Result |
| --- | --- |
| `cargo test -p druid-wrapper --features sqlx --lib cancelled_next_keeps_the_pending_row_for_the_next_call --locked` | 1 passed; 21 filtered out |
| `cargo fmt -p druid-wrapper -- --check` | Exit 0; formatting clean |
| `cargo test -p druid-wrapper --features sqlx --lib sqlite_live_resources_capabilities_and_async_result_set_are_truthful --locked` | 1 passed; 21 filtered out |
| `openspec validate p04-task7-result-set-stream-adapter --strict --no-interactive` | Exit 0; change valid |
| `git diff --check` | Exit 0; no whitespace errors |

The test commands used `CARGO_HOME`, `CARGO_TARGET_DIR`, `TEMP`, and `LOCALAPPDATA` under `E:\druid-rdbc-task6-isolated` to keep generated files on the permitted E: drive.

## Limits

This change does not add adapter features or alter production behavior. The cancellation regression covers the SQLx cursor's pending-response ownership path. Live database assertions cover SQLx SQLite only. It does not establish JDBC TCK conformance, resource support on other backends, or project-wide production readiness.
