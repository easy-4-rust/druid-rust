| Plan | Status | Evidence |
| :--- | :--- | :--- |
| external-pool-rdbc-conformance | COMPLETE | Commits `a75eb8d..fd70a60`; final review CLEAR. Configured SQLite exact gates exit 0: direct SQLx/bb8/deadpool shared contracts 1 passed each; wrapper tests exit 0; recycle semantics 12 passed; prepared cache 6 passed. PostgreSQL/MySQL/Toasty/RBDC/JDBC Agent remain UNVERIFIED. Supplemental review: `.superpowers/sdd/2026-08-21-external-pool-rdbc-conformance/progress.md`. |
| metrics-admin-single-protocol | COMPLETE | Commits `6e363c2..80c534f`, completion HEAD `80c534f`. Fresh full package gates exit 0: `cargo test -p druid-metrics --quiet` 115 tests + 1 doctest; `--all-features` 116 tests + 1 doctest; `cargo test -p druid-admin --quiet` 119 tests; `--all-features` 119 tests. Key named/Clippy/proto gates remain recorded in supplemental reports. P07 deferred: pre-publication `PushEvent`/`PushWorkerExit`/DTO/`DataSourceId` shapes and same-identity stale-guard mutation coverage. |
| druid-rdbc-extraction | NOT_STARTED | P01 prerequisite is complete; API baseline remains `bf88754fefc9c57ae1519e93f70f0db7118747dc3a1334156ec93778beac94b6` (see `baseline.md`). |
| rdbc-contract-hardening | BLOCKED_BY_P03 | -- |
| driver-registry-feature-boundaries | BLOCKED_BY_P03 | -- |
| runtime-production-hardening | BLOCKED_BY_P04 | P02 prerequisite is complete at `80c534f`; P04 remains outstanding. |
| rdbc-first-verification | BLOCKED_BY_P01_P06 | -- |
