# P04 Task7: SQLx ResultSet cancellation and resource capability evidence

## Why

The SQLx adapter already uses a bounded asynchronous cursor and truthfully reports the connection-level capabilities exercised by its SQLite backend. Its `next` implementation intentionally retains an in-flight response when the caller cancels the future, but that lifecycle contract had no focused regression test. The existing live SQLite evidence verifies ordered rows, EOF, close, and unsupported resource factories, but does not verify cancellation recovery.

## Scope

- Verify cancellation of a pending `ResultSet::next` does not lose the eventual row.
- Retain the existing live SQLx SQLite evidence for capability truth, unsupported resource factories, row order, EOF, and explicit close.
- Record the exact limits of this evidence.

## Non-goals

- No change to SQLx cursor behavior or public API.
- No claim that SQLite supports Array, Blob, Clob, NClob, SQLXML, or Struct.
- No claim that this verifies Toasty, JDBC Agent, PostgreSQL, MySQL, DuckDB, or every JDBC contract.

## Acceptance

- A deterministic async unit test cancels a pending `next`, completes the same pending response, and verifies the subsequent `next` receives that row.
- The focused SQLx SQLite integration test continues to pass against `sqlite::memory:`.
- Formatting, strict OpenSpec validation, and `git diff --check` pass.
