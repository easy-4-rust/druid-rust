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

## Fix round 1/5 — three Important findings

Review baseline: `0d83f229275301f0e81736e4b6d8f82dc799a11d`.

This round remained inside P04 Task 1. It did not rename standard traits, remove `Physical*`,
start Task 2, install tools, switch branches, create a worktree, or push/publish.

### RED evidence

#### Important 1 — real batch paths lost large counts

Default standard path:

```bash
cargo test -p druid-rdbc --test error_contract_test \
  default_batch_path_preserves_large_failure_counts_and_java_success_view -- --exact
```

Observed exit `101`, runtime assertion:

```text
left:  [2147483647]
right: [2147483654]
0 passed; 1 failed
```

Real RBDC adapter path:

```bash
cargo test -p druid-wrapper --test rbdc_connection_adapter_test \
  rbdc_adapter_batch_path_preserves_large_failure_counts_and_java_success_view -- --exact
```

After correcting the fixture to use the adapter's real `add_batch` state path, the observed target
failure was:

```text
left:  [2147483647]
right: [2147483654]
0 passed; 1 failed
```

Both failures occurred only after a real successful execution returned `i32::MAX + 7` and the next
batch item failed. They did not rely on manually constructing `BatchUpdateCounts`.

#### Important 2 — product errors collapsed to `Other`

```bash
cargo test -p druid --test pool_maintenance_differential_test \
  pool_trait_get_preserves_fatal_product_class_and_typed_source -- --exact
```

Observed exit `101`, runtime assertion:

```text
left:  "rdbc::Other"
right: "java.sql.SQLException"
0 passed; 1 failed
```

The test executes the real `onFatalErrorMaxActive` transition, keeps two active borrows, and calls
`<DruidPool as Pool>::get`; it therefore covers the actual product-to-RDBC pool boundary.

#### Important 3 — wrapped batch accessor was opaque

```bash
cargo test -p druid --test druid_error_transition_test \
  wrapped_rdbc_batch_counts_delegate_all_three_views -- --exact
```

Observed exit `101`, expected contract compilation failures:

```text
no method named `large` found for reference `&[i32]`
no method named `jdbc_i32` found for reference `&[i32]`
no method named `try_i32` found for reference `&[i32]`
```

The failure showed that the old accessor shape could neither delegate `DruidError::Rdbc` nor expose
the three required views.

### Fix implementation

1. `BatchUpdateCounts::try_push_rows_affected` now converts driver `u64` counts to JDBC signed
   `i64` with a checked upper bound. Default `exec_batch` / `exec_prepared_batch` and the Toasty,
   SQLx, and RBDC adapter batch loops accumulate `BatchUpdateCounts` directly. Failure paths retain
   the original `i64`; successful APIs still return `jdbc_i32()` narrowing-cast compatibility.
2. Added generic `ProductErrorBoundary { class_name, message, typed source }` under `RdbcError`.
   `From<DruidError> for RdbcError` uses it for product-only errors instead of `Other`; the type does
   not copy Druid product variants into the RDBC taxonomy. `Display`, `class_name`, `Error::source`,
   SQL exception lookup, and batch lookup delegate through the typed boundary.
3. Druid product batch payloads now use `BatchUpdateCounts`. `DruidError::batch_update_counts`
   transparently returns the product payload or delegates `DruidError::Rdbc`, allowing `large()`,
   `jdbc_i32()`, and `try_i32()` on the same accessor result.
4. The waiver verifier now recognizes both direct and grouped named re-exports, so exporting
   `ProductErrorBoundary` beside `RdbcError` does not create a false gate failure.

### GREEN evidence

| Command | Result | Tests |
| --- | --- | ---: |
| `cargo test -p druid-rdbc --test error_contract_test` | PASS | 6 passed |
| `cargo test -p druid --test druid_error_transition_test` | PASS | 4 passed, including all 24 P03 goldens |
| `cargo test -p druid --test exception_sorter_semantics_test` | PASS | 10 passed |
| `cargo test -p druid-wrapper --test adapter_boundary_test` | PASS | 4 passed |
| `cargo test -p druid-wrapper --test rbdc_connection_adapter_test` | PASS | 10 passed |
| `cargo test -p druid --test pool_maintenance_differential_test` | PASS | 10 passed |
| **Executed focused total** | **PASS** | **44 passed, 0 failed, 0 ignored** |
| waiver verifier | PASS | all six exact gates; internally reruns the 6 + 4 error tests |
| `cargo check --workspace --all-targets --all-features` | PASS | compile-only; 0 tests |
| `cargo fmt --all -- --check` | PASS | format-only; 0 tests |

Final live denominator remains `90` rows with SHA-256
`7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603`.

### Fix-round files

- `crates/druid-rdbc/src/error/{batch_update_counts.rs,rdbc_error.rs,rdbc_error_kind.rs,mod.rs}`
- `crates/druid-rdbc/src/spi/physical_connection.rs`
- `crates/druid-rdbc/tests/error_contract_test.rs`
- `crates/druid/src/core/error.rs`
- `crates/druid/tests/{druid_error_transition_test.rs,pool_maintenance_differential_test.rs,druid_error_test.rs}`
- `crates/druid-wrapper/src/{toasty/toasty_connection_adapter.rs,sqlx/sqlx_connection_adapter.rs,rbdc/rbdc_connection_adapter.rs}`
- `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- `scripts/verify_rdbc_error_waiver_exit.py`
- `docs/verification/rdbc-first/rdbc-error-waiver-exit.md`
- this report

### Deferred Minor findings

The two reviewer Minor findings remain intentionally deferred in the authoritative SDD ledger and
were not changed in this round:

1. `compat/exceptions.rs` still describes Java narrowing as saturation; final review owns the
   wording decision.
2. This report remains force-tracked despite the SDD workspace ignore rule; final review owns the
   tracked-history decision.

They do not weaken the three Important regressions or the executable gates above.

## Fix round 2/5 — canonical export verifier fail-closed

Review baseline: `743632143965b13dce55a7ca9a6e2f17daf43797`.

This round changed only `scripts/verify_rdbc_error_waiver_exit.py` and this report. No production
Rust file, generated verification report schema, Task 2 artifact, `Physical*` name, dependency, or
Cargo configuration changed. The force-tracked report Minor remains deferred as instructed.

### RED evidence

Self-test command:

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py --self-test
```

Before the parser fix, the command exited `1`. Exact direct and simple grouped positive cases
passed, while the nested-depth positive and every adversarial target form exposed the fail-open:

```text
test_accepts_exact_direct_export ... ok
test_accepts_exact_grouped_export (nested unrelated group before canonical item) ... FAIL
test_rejects_direct_alias ... FAIL
test_rejects_direct_and_grouped_child_paths (direct) ... FAIL
test_rejects_direct_and_grouped_child_paths (grouped) ... FAIL
test_rejects_grouped_alias_and_underscore (Hidden) ... FAIL
test_rejects_grouped_alias_and_underscore (_) ... FAIL
test_rejects_nested_grouped_export ... FAIL
Ran 6 tests
FAILED (failures=7)
```

All negative failures were `AssertionError: True is not false`, proving the verifier accepted a
non-canonical target rather than merely failing on unrelated syntax.

### Parser correction

- Direct exports now require the full token entry `pub use rdbc_error::RdbcError;`; any token after
  `RdbcError` other than `;` rejects the occurrence, including `as`, `_`, and `::Child`.
- Grouped exports are split only on commas at the outer group depth. Nested brace depth is tracked
  until the matching outer `}` and the complete grouped item must end in `;`.
- A canonical grouped entry must be exactly `[RdbcError]`. Alias, underscore, child, or nested
  entries containing the target mark the source invalid even if a separate accepted occurrence is
  present, keeping Gate2 fail-closed.
- An unrelated nested group before an exact top-level `RdbcError` entry is accepted, proving the
  parser does not stop at the first nested closing brace.

### GREEN evidence

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py --self-test
```

```text
test_accepts_exact_direct_export ... ok
test_accepts_exact_grouped_export ... ok
test_rejects_direct_alias ... ok
test_rejects_direct_and_grouped_child_paths ... ok
test_rejects_grouped_alias_and_underscore ... ok
test_rejects_nested_grouped_export ... ok
Ran 6 tests in 0.000s
OK
```

Canonical waiver command:

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv \
  --exit-report docs/verification/rdbc-first/rdbc-error-waiver-exit.md
```

Final result: all six exact `WAIVER_EXIT_GATE_*` values `PASS`; Gate6 internally reran
`error_contract_test` and `druid_error_transition_test`; live rows remained `90` and manifest
SHA-256 remained `7f19e173dcadd1ea6b55cf24e4be61ee3de2ab8cca650399617a773823762603`.

Per the fix-round instruction, no workspace broad check was required because no production or
generated-schema input changed.
