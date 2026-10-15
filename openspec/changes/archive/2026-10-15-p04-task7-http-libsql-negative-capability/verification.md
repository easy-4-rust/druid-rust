# Verification

- `cargo test -p druid-wrapper --no-default-features --features driver-catalog,http-sql,libsql-native --lib --locked --offline`: 10 passed, 0 failed.
- The focused `standard_factory_uses_deterministic_transport_owned_core` filter: 2 passed, 0 failed.
- `openspec validate 2026-10-15-p04-task7-http-libsql-negative-capability --strict`: passed.
- `rustfmt --edition 2021 --check` on the two changed Rust files: passed.
- `git diff --check`: passed.

The deterministic transports verify standard adapter semantics but do not establish live rqlite, Cloudflare D1, or remote libSQL interoperability. The full Task 7 backend matrix remains open.
