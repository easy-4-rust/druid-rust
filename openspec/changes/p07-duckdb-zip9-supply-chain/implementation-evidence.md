# Verification evidence

- CI run `38981435014` on pre-upgrade commit `5535d81` completed with 43 successful jobs and one failing `supply-chain` job. Its `cargo audit` step reported `RUSTSEC-2026-0367`, `0368`, and `0369` against `zip 6.0.0`.
- The committed baseline graph was `druid-wrapper -> duckdb 1.10505.0 -> libduckdb-sys 1.10505.0 -> zip 6.0.0` (build dependency). Upstream `libduckdb-sys` called `ZipArchive::extract`.
- The proposed graph resolves `duckdb/libduckdb-sys 1.10506.1 -> zip 9.0.4` with `cargo +1.97.1 update -p duckdb --precise 1.10506.1`; `cargo tree --locked --all-features -i zip@9.0.4 -p druid-wrapper` confirmed it. DuckDB's two adapter tests passed on Windows.
- The obsolete CI-only override of `zip 6.0.0` in the SemVer job was removed; the SQLite compatibility override remains.

Still required: clean audit/deny CI, full affected feature and cross-platform checks, and review of any new failures. No waiver was added.
