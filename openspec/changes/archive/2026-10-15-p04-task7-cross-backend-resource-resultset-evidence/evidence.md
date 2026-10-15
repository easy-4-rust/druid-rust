# Evidence

Local Windows, 2026-10-15:

- `cargo test -p druid-wrapper --all-features --test capability_truthfulness_live_test sqlite_capability_claims_match_committed_rows -- --exact`: passed, 1 test. This exercises both SQLx and Toasty against actual in-memory SQLite connections.
- `cargo test -p druid-wrapper --no-default-features --features driver-catalog,sqlx,sqlite --test capability_truthfulness_live_test sqlite_capability_claims_match_committed_rows -- --exact`: passed, 1 test. This also proves the contract under the minimal SQLite feature set.
- `rustfmt --edition 2021 --check crates/druid-wrapper/tests/capability_truthfulness_live_test.rs`: passed. Workspace-wide `cargo fmt --all -- --check` could not start on Windows because the command exceeded OS path/argument length (`os error 206`); the changed Rust test file was formatted and checked directly.
- `openspec validate p04-task7-cross-backend-resource-resultset-evidence --strict`: passed.
- `git diff --check`: passed.

The pushed `e87aad6` [driver-matrix run](https://github.com/easy-4-rust/druid-rust/actions/runs/38670343887) executed the same backend-neutral test on live PostgreSQL and MySQL:

- [PostgreSQL real product contract](https://github.com/easy-4-rust/druid-rust/actions/runs/38670343887/job/116211860280): `cargo test -p druid-wrapper --no-default-features --features driver-catalog,sqlx,postgresql --test capability_truthfulness_live_test -- --ignored --exact live_backend_capability_claims_match_committed_rows`; log: `test live_backend_capability_claims_match_committed_rows ... ok`, 1 passed.
- [MySQL real product contract](https://github.com/easy-4-rust/druid-rust/actions/runs/38670343887/job/116211860202): the same exact test with `--features driver-catalog,sqlx,mysql`; log: 1 passed.

Both tests instantiate SQLx and Toasty factories within the shared test function. At evidence capture, the full matrix was still in progress; its other platform results are not accepted here. This evidence does not cover JDBC Agent, RBDC, DuckDB, or every JDBC method.
