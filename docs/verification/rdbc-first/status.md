| Plan | Status | Evidence |
| :--- | :--- | :--- |
| external-pool-rdbc-conformance | COMPLETE | Commits `a75eb8d..fd70a60`; final review CLEAR. Configured SQLite exact gates exit 0: direct SQLx/bb8/deadpool shared contracts 1 passed each; wrapper tests exit 0; recycle semantics 12 passed; prepared cache 6 passed. PostgreSQL/MySQL/Toasty/RBDC/JDBC Agent remain UNVERIFIED. Supplemental review: `.superpowers/sdd/2026-08-21-external-pool-rdbc-conformance/progress.md`. |
| metrics-admin-single-protocol | COMPLETE | Commits `6e363c2..80c534f`, completion HEAD `80c534f`. Fresh package gates: Metrics default exit 0 (115 tests + 1 doctest), all-features exit 0 (116 tests + 1 doctest), Admin default exit 0 (119 tests). Admin all-features had one same-command cold-start run exit 101 (4 startup timing tests at the 2s deadline), then the same command fresh/cached rerun exit 0 (119/119); investigation is evidence of build/large-binary cold-start resource contention, not a determined root cause or proof of product stability. Key named/Clippy/proto gates remain recorded in supplemental reports. P07 deferred: pre-publication `PushEvent`/`PushWorkerExit`/DTO/`DataSourceId` shapes and same-identity stale-guard mutation coverage. |
| druid-rdbc-extraction | NOT_STARTED | P01 prerequisite is complete; API baseline remains `bf88754fefc9c57ae1519e93f70f0db7118747dc3a1334156ec93778beac94b6` (see `baseline.md`). |
| rdbc-contract-hardening | BLOCKED_BY_P03 | -- |
| driver-registry-feature-boundaries | BLOCKED_BY_P03 | -- |
| runtime-production-hardening | BLOCKED_BY_P04 | P02 prerequisite is complete at `80c534f`; P04 remains outstanding. |
| rdbc-first-verification | BLOCKED_BY_P01_P06 | -- |
