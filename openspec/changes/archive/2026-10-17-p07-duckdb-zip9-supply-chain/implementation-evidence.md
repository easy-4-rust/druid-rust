# Verification evidence

- CI run `38981435014` on pre-upgrade commit `5535d81` completed with 43 successful jobs and one failing `supply-chain` job. Its `cargo audit` step reported `RUSTSEC-2026-0367`, `0368`, and `0369` against `zip 6.0.0`.
- The committed baseline graph was `druid-wrapper -> duckdb 1.10505.0 -> libduckdb-sys 1.10505.0 -> zip 6.0.0` (build dependency). Upstream `libduckdb-sys` called `ZipArchive::extract`.
- The proposed graph resolves `duckdb/libduckdb-sys 1.10506.1 -> zip 9.0.4` with `cargo +1.97.1 update -p duckdb --precise 1.10506.1`; `cargo tree --locked --all-features -i zip@9.0.4 -p druid-wrapper` confirmed it. DuckDB's two adapter tests passed on Windows.
- The obsolete CI-only override of `zip 6.0.0` in the SemVer job was removed; the SQLite compatibility override remains.

The initial upgrade still required clean audit/deny CI, full affected feature and cross-platform checks; the final results are recorded below. No waiver was added.

CI run `38988298868` exposed a lockfile reproducibility error before audit: a developer-local global Cargo patch had written an unused `zip 6.0.0` patch record into `Cargo.lock`. Regenerating with a clean D-drive `CARGO_HOME` removed only that unused record. `cargo metadata --format-version 1 --locked` and `scripts/verify_rsa_waiver_scope.py` then passed in the clean environment; the final CI audit also passed, as recorded below.

Local follow-up on `e7cd955`: `cargo +1.97.1 check --locked -p druid-wrapper --all-features` passed using a D-drive target directory. `cargo +1.97.1 test --locked -p druid-wrapper --features duckdb-native -- --test-threads=1` passed all 218 tests across 42 test targets, including the DuckDB adapter. The E-drive target had previously exhausted available disk space while linking; the same source passed after redirecting build artifacts to D. CI run `38988639237` on `e7cd9557bdcd6d5933ac29ef4ba5fcd18d817e28` completed successfully: all 44 jobs passed, including `supply-chain` (RSA waiver scope, audit, and deny), `public-api`, `semver`, Rust contracts, and the cross-platform DuckDB/SQLite/JDBC Agent matrix. No advisory waiver was added.
