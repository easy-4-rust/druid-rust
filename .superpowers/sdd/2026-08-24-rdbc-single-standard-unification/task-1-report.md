# P04 Task 1 — `DruidError` waiver exit report

## Status and scope

- Status: `DONE`
- Specification: `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`, ADR-RDBC-006 §10–§13.
- Task brief: `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-1-brief.md`.
- Frozen baseline: `bb91193d9bb488b107b8c9ac6dd56f8756ed9593`.
- Initial worktree: clean; initial `HEAD` matched the frozen baseline.
- Scope retained: error ownership, signatures, constructions, and compatibility evidence only. No standard trait rename, `Physical*` deletion, resource internalization, behavior hardening, branch switch, worktree, push, publish, tool installation, or SemVer command.

## RED evidence

### RED 1 — live disposition and waiver gates

Commands:

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py --emit-live \
  docs/verification/rdbc-first/rdbc-error-disposition.tsv
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv
```

Observed exit: `1` on the verifier command. Key output:

```text
LIVE_ROWS=90
LIVE_SHA256=7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603
WAIVER_EXIT_GATE_1_COMPAT_REMOVED=FAIL still exists: crates/druid-rdbc/src/compat/druid_error.rs; live DruidError remains in 39 druid-rdbc file(s)
WAIVER_EXIT_GATE_2_STANDARD_CLEAN=FAIL 39 standard file(s) still use DruidError; RdbcError is not a public enum in error/rdbc_error.rs
WAIVER_EXIT_GATE_3_ADAPTERS_CLEAN=FAIL 51 adapter file(s) still use DruidError; wrapper has no live RdbcError production rows
WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE=FAIL DruidError definitions are not uniquely owned by core/error.rs
WAIVER_EXIT_GATE_5_BASELINE_DISPOSED=FAIL not deleted: crates/druid-rdbc/src/compat/druid_error.rs; baseline rows remain
WAIVER_EXIT_GATE_6_COMPAT_VERIFIED=FAIL ownership/disposition prerequisites are not green
```

The denominator is Rust-token aware: comments and literals do not create rows. It is regenerated from the frozen Git baseline, not from a fixed file count.

### RED 2 — RDBC fidelity contract

Command:

```bash
cargo test -p druid-rdbc --test error_contract_test
```

Observed exit: `101`. Key failures:

```text
unresolved imports druid_rdbc::error::BatchUpdateCounts, druid_rdbc::error::RdbcErrorKind
no associated function or constant named SqlException found for struct RdbcError
no associated function or constant named feature_not_supported found for struct RdbcError
no associated function or constant named DriverError found for struct RdbcError
```

This was the expected failure because `RdbcError` was still the non-constructible scaffold and batch counts had no lossless `i64` value object.

### RED 3 — Druid product ownership and wrapper

Command:

```bash
cargo test -p druid --test druid_error_transition_test
```

Observed exit: `101`. Key failures:

```text
no variant or associated item named Rdbc found for enum DruidError
no associated function or constant named DriverError found for struct RdbcError
```

The P03 test file was renamed before production implementation and retained all 24 independent golden cases; the new assertion required distinct product/standard types and exactly one product wrapper.

## Implementation and compatibility result

- Added `BatchUpdateCounts`, storing only `Vec<i64>`, with:
  - `large()` lossless view;
  - `jdbc_i32()` Java narrowing-cast view;
  - `try_i32()` checked Rust view;
  - `len()` / `is_empty()` and `Vec<i32>` / `Vec<i64>` conversions.
- Replaced the `RdbcError` scaffold with the standard/adapter error enum and `RdbcErrorKind` taxonomy.
- Preserved SQLState, vendor code, class/assignability, recoverability, cause classification, next-exception chain, warning chain, client-info failed properties, `DataTruncation`, typed Rust `Error::source`, and `i64` batch counts.
- Kept SQL feature unsupported (`SQLFeatureNotSupportedException`, SQLState `0A000`) distinct from the JDBC default `UnsupportedOperationException` mapping.
- Moved the sole `DruidError` definition back to `crates/druid/src/core/error.rs`; removed `crates/druid-rdbc/src/compat/druid_error.rs` and its export.
- Added `DruidError::Rdbc(RdbcError)` plus lossless shared-category conversions. An already wrapped `DruidError::Rdbc` unwraps back to the same `RdbcError`, preventing nested product wrappers.
- Migrated every frozen `druid-rdbc` standard and `druid-wrapper` adapter production row to `RdbcError`, then migrated the Druid logical/Filter boundary conversions required by compilation.
- Retained the P03 24-variant product golden observables byte-for-behavior: display, class name, SQL payload, batch view, and product source chain.

## Live row count and hash

- Data rows: `90`.
- Domain rows: `RDBC_COMPAT=2`, `RDBC_STANDARD=37`, `WRAPPER_ADAPTER=51`.
- Manifest SHA-256: `7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603`.
- Authoritative row-by-row file list and disposition: `docs/verification/rdbc-first/rdbc-error-disposition.tsv`.
- Exit artifact: `docs/verification/rdbc-first/rdbc-error-waiver-exit.md`.

## Six exact waiver gates

Final verifier command:

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv \
  --exit-report docs/verification/rdbc-first/rdbc-error-waiver-exit.md
```

Final exit: `0`.

```text
WAIVER_EXIT_GATE_1_COMPAT_REMOVED=PASS
WAIVER_EXIT_GATE_2_STANDARD_CLEAN=PASS
WAIVER_EXIT_GATE_3_ADAPTERS_CLEAN=PASS
WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE=PASS
WAIVER_EXIT_GATE_5_BASELINE_DISPOSED=PASS
WAIVER_EXIT_GATE_6_COMPAT_VERIFIED=PASS
LIVE_ROWS=90
LIVE_SHA256=7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603
```

## GREEN commands and test counts

| Command | Result | Executed tests |
| --- | --- | ---: |
| `cargo test -p druid-rdbc --test error_contract_test` | PASS | 5 passed, 0 failed, 0 ignored |
| `cargo test -p druid --test druid_error_transition_test` | PASS | 3 passed, 0 failed, 0 ignored |
| `cargo test -p druid --test exception_sorter_semantics_test` | PASS | 10 passed, 0 failed, 0 ignored |
| `cargo test -p druid-wrapper --test adapter_boundary_test` | PASS | 4 passed, 0 failed, 0 ignored |
| **Focused total** | **PASS** | **22 passed, 0 failed, 0 ignored** |
| `cargo check --workspace --all-targets --all-features` | PASS, 0 warning | 0 tests; compile-check only |
| `cargo fmt --all -- --check` | PASS | 0 tests; format-check only |
| final waiver verifier | PASS | reruns the 5 + 3 focused error tests internally |

The workspace all-target/all-feature command compiled all test targets but did not execute their test bodies. No full workspace test suite, real database matrix, Java live differential, MSRV rerun, Clippy, doc test, or SemVer check was claimed by this task.

## Files changed

The worktree has `200` status rows before staging. The exact production denominator is the 90-row TSV cited above. The change families are:

1. Error ownership and value contract:
   - `crates/druid-rdbc/src/error/{batch_update_counts.rs,rdbc_error.rs,rdbc_error_kind.rs,sql_exception.rs,mod.rs}`;
   - `crates/druid-rdbc/src/compat/{druid_error.rs,exceptions.rs,mod.rs}` (`druid_error.rs` deleted);
   - `crates/druid/src/core/{error.rs,mod.rs}`.
2. Frozen standard/adapter rows:
   - all `RDBC_COMPAT`, `RDBC_STANDARD`, and `WRAPPER_ADAPTER` paths listed in `rdbc-error-disposition.tsv`.
3. Druid logical/product conversion boundary:
   - affected `crates/druid/src/core`, `dynamic`, `pool`, `spi`, and `sql` error signatures/constructions required to implement or call the same RDBC contracts.
4. Compile-contract fixtures:
   - affected integration tests under `crates/druid/tests` and `crates/druid-wrapper/tests`; changes are error-type/signature/construction migrations plus `BatchUpdateCounts::large()` assertions.
5. New/renamed executable evidence:
   - `crates/druid-rdbc/tests/error_contract_test.rs`;
   - `crates/druid/tests/druid_error_transition_test.rs` (renamed from `druid_error_transition_identity_test.rs` and adapted without dropping its 24 goldens);
   - `scripts/verify_rdbc_error_waiver_exit.py`;
   - `docs/verification/rdbc-first/rdbc-error-disposition.tsv`;
   - `docs/verification/rdbc-first/rdbc-error-waiver-exit.md`;
   - this report.

For exact review after commit, use:

```bash
git diff --name-status bb91193d9bb488b107b8c9ac6dd56f8756ed9593..HEAD
```

Before the report was added, the tracked diff was 192 paths with 6,431 additions / 6,254 deletions; the completed worktree then had 8 untracked paths. The final staged set is recorded by `git diff --cached --name-status` during the commit gate.

## Self-review and concerns

- Confirmed scope contains no `Physical*` rename/removal, resource hardening, branch/worktree, push, publish, or tool installation.
- Confirmed `DruidError` is defined once in Druid and no live `DruidError` token remains in `druid-rdbc/src` or `druid-wrapper/src`.
- Confirmed all 90 baseline rows are derived and validated against Git object bytes at the frozen baseline; deleted/missing/extra baseline rows fail closed.
- Confirmed `RdbcError::SqlException` and batch cause implement typed `Error::source` behavior and the two unsupported taxonomies remain distinct.
- Confirmed the newly renamed 608-line Druid transition test is cohesive: one frozen 24-variant golden matrix plus its shared observation helpers. It remains below the 800-line blocker and is intentionally kept together so variant observables cannot drift across files.
- Concern, not a Task 1 blocker: 29 pre-existing authored Rust files touched mechanically by the signature migration remain above 800 physical lines. Task 1 did not create these large files or authorize unrelated decomposition; the task-specific compiler and focused behavior gates pass. Their structural decomposition remains separate migration-governance work.
- Concern, explicitly unverified: compile-only migration of the wider existing test matrix does not establish that every migrated test body passes. This report claims only the 22 executed focused tests and the all-target/all-feature compile gate required by the brief.

## Completion

- Corresponding change: P04 Task 1 / ADR-RDBC-006 error waiver exit.
- Implemented: unified RDBC/adapter error contract, unique Druid product owner, live denominator/disposition, six exact gates, fidelity and transition contracts.
- Unfinished inside Task 1: none.
- Current specification state: Task 1 executable evidence is GREEN; later P04 standard trait merge/delete tasks remain untouched.
- Next action: independent review/fix loop, then P04 Task 2 only after this commit is accepted.
