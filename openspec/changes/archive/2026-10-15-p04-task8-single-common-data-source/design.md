# Design

Replace the legacy trait definition in `druid/src/sql/common_data_source.rs` with exact re-exports of `CommonDataSource` and `RdbcLogWriter`. Keep `druid::sql::DataSource` as a product-specific connection facade but let its supertrait resolve to the canonical common contract. Remove duplicate implementations where a standard implementation already exists. Add default canonical implementations for dynamic data sources that had only the legacy empty implementation. Delegate wrapper logging to the pool's canonical implementation. Update tests to use `try_login_timeout`, `Duration`, and the standard `0A000` unsupported-parent-logger result. The old integer-seconds methods are removed as an approved pre-release API migration, not silently emulated.

The final purity verifier and its public-contract gate launch Python children. Resolve both with `sys.executable` so the same verified interpreter runs on Windows and Linux; unit assertions cover both invocation choices.
