# Evidence

Local Windows, 2026-10-15:

- `cargo test -p druid-wrapper --all-features --test capability_truthfulness_live_test sqlite_capability_claims_match_committed_rows -- --exact`: passed, 1 test. This exercises both SQLx and Toasty against actual in-memory SQLite connections.
- `rustfmt --edition 2021 --check crates/druid-wrapper/tests/capability_truthfulness_live_test.rs`: passed. Workspace-wide `cargo fmt --all -- --check` could not start on Windows because the command exceeded OS path/argument length (`os error 206`); the changed Rust test file was formatted and checked directly.
- `openspec validate p04-task7-cross-backend-resource-resultset-evidence --strict`: passed.
- `git diff --check`: passed.

PostgreSQL and MySQL execution remains pending until the pushed commit's existing driver-matrix CI jobs complete. This evidence does not cover JDBC Agent, RBDC, DuckDB, or every JDBC method.
