# Verification

- RED: `cargo test -p druid-wrapper --test data_source_credential_hardening_test --locked --offline` failed with E0277 because `DruidRdbcDataSource` had no `Debug` implementation. A first fixture import error was corrected before recording this RED result.
- GREEN: `cargo test -p druid-wrapper --features sqlx --test data_source_credential_hardening_test --test task6_rdbc_data_source_test_parity --test common_data_source_logging_test --test common_data_source_public_examples_test --locked --offline --quiet` passed 7/7 tests, including the unconfigured-credential case.
- `cargo clippy -p druid-wrapper --lib --features sqlx --locked --offline -- -D warnings` passed.
- `rustfmt --edition 2021 --check` on both changed Rust files and `openspec validate 2026-10-15-rdbc-datasource-credential-hardening --strict` passed.
- `python scripts/verify_wrapper_standard_impls.py` ran but failed in the unrelated HTTP_SQL exact-test compiler coverage identity: `positive identities=[]`, expected `['HTTP_SQL']`. This Windows-local failure does not certify that broad gate; CI must provide the final cross-platform result.
- `cargo clippy -p druid-wrapper --all-targets --all-features --locked --offline -- -D warnings` failed in pre-existing `jdbc_agent_xa_connection.rs` and `jdbc_agent_xa_external_consumer.rs` test assertions (`manual_assert_eq` and `manual_assert`). The changed library passed its targeted strict Clippy gate. The two broader warnings remain open P07 quality work.

The Task 8 final JDBC semantic inventory remains incomplete; this change does not promote its planned rows or claim production readiness.

## CI correction

Run `38728847589` exposed two omissions in the first submission: the new test's target metadata required `sqlx` but not the `driver-catalog` module it imports, and the pinned macOS public-API snapshot omitted the newly implemented `Debug` trait. The target now requires both features. The wrapper snapshot and SHA-256 were copied from that run's `public-api-observed` artifact; comparison with the previous snapshot showed only the two expected `Debug` declarations. Local `cargo check -p druid-wrapper --no-default-features --features sqlx --tests --locked --offline --quiet` and `cargo test -p druid-wrapper --no-default-features --features driver-catalog,sqlx --test data_source_credential_hardening_test --locked --offline --quiet` both exited successfully (the latter 2/2 tests). The corrected commit still requires a fresh CI result.
