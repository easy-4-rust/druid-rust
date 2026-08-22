| Plan | Status | Evidence |
| :--- | :--- | :--- |
| external-pool-rdbc-conformance | COMPLETE | Commits `a75eb8d..fd70a60`; final review CLEAR in `.superpowers/sdd/2026-08-21-external-pool-rdbc-conformance/progress.md` and `final-review-a75eb8d..fd70a60.diff`; fresh focused contract gates exit 0 (direct SQLx/bb8/deadpool 1 passed each), wrapper tests exit 0, recycle semantics 12 passed, prepared cache 6 passed. |
| metrics-admin-single-protocol | COMPLETE | Commits `6e363c2..80c534f`; completion and clean residual re-review in `.superpowers/sdd/2026-08-21-metrics-admin-single-protocol/progress.md`; fresh final gates in `residual-task-report.md`: Metrics default 115 + 1 doctest, all-features 116 + 1 doctest; Admin default/all-features 119 tests; both Clippy gates exit 0. |
| druid-rdbc-extraction | NOT_STARTED | P01 prerequisite is complete; API baseline remains `bf88754fefc9c57ae1519e93f70f0db7118747dc3a1334156ec93778beac94b6` (see `baseline.md`). |
| rdbc-contract-hardening | BLOCKED_BY_P03 | -- |
| driver-registry-feature-boundaries | BLOCKED_BY_P03 | -- |
| runtime-production-hardening | BLOCKED_BY_P04 | P02 prerequisite is complete at `80c534f`; P04 remains outstanding. |
| rdbc-first-verification | BLOCKED_BY_P01_P06 | -- |
