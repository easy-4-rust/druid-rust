# Task 6 implementation report — single-standard breaking cutover

Status: **IMPLEMENTED_PENDING_INDEPENDENT_REVIEW**

BASE: `5afe8609b177544ff7d18461aa84f69efe8ee90e`

Specification: `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`

Task brief: `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-6-brief.md`

Task 5 handoff: REVIEW CLEAR at `b115287`; checked Task 6 start at `5afe860`.

## 1. Outcome

Task 6 performs the planned atomic breaking cutover:

- `druid-rdbc` exposes one canonical standard trait tree and no `spi` module.
- Public API, production Rust and rustdoc contain zero `Physical[A-Z]` names.
- Druid and wrapper contain no private duplicate trait tree or compatibility aliases.
- Frozen Task 6 consumers are `75 CLOSED / 0 ACTIVE`; Task 5 remains `180 CLOSED`; Task 7 remains `108 PENDING`.
- All 79 old production/public identities have one approved disposition and a real target where required.
- All 67 deleted integration-test paths have one checked disposition and existing canonical replacement tests where behavior remains.
- Task 4 runtime/compiler evidence was rebuilt against the final code and passes seven exact families.
- JDBC 4.2 semantic inventory remains `java.sql=52 / javax.sql=17 / UNKNOWN=0`.

No branch was created or switched, no worktree was used, and nothing was pushed.

## 2. TDD evidence

The final gate and external-consumer test were written before the cutover:

- `scripts/test_verify_no_physical_standard.py`
- `scripts/verify_no_physical_standard.py`
- `crates/druid-rdbc/tests/no_physical_public_surface_test.rs`

Fresh RED was observed on the BASE-compatible tree:

- the repository verifier rejected the live `druid_rdbc::spi` and retired identities;
- the negative external consumer unexpectedly compiled because the old compatibility API still existed.

Final GREEN:

```text
python3 scripts/test_verify_no_physical_standard.py
Ran 6 tests ... OK

python3 scripts/verify_no_physical_standard.py
PRODUCTION_RUST_FILES_SCANNED=599
PUBLIC_PHYSICAL_NAMES=0
PRODUCTION_PHYSICAL_NAMES=0
RUSTDOC_PHYSICAL_NAMES=0
DRUID_RDBC_SPI_MODULES=0
MIGRATION_IDENTITIES_CLOSED=79
NEWLY_DISCOVERED_IDENTITIES_CLOSED=11
TRANSITIONAL_CONSUMER_IDENTITIES_CLOSED=75
DELETED_TEST_DISPOSITIONS=67
ACTIVE_TRANSITIONAL_CONSUMERS=0

cargo test -p druid-rdbc --test no_physical_public_surface_test
1 passed; 0 failed
```

The verifier derives production modules and Cargo features dynamically, scans public API and generated rustdoc, rejects aliases/re-exports/macros/cfg/path/include tricks, checks the reachable module graph for a duplicate trait tree, authenticates CLOSED75 against BASE, and derives deleted tests from the BASE diff.

## 3. Migration disposition

`docs/rdbc/physical-to-standard-migration.md` is the exact human-readable migration contract.

| denominator | result |
| --- | ---: |
| production/public old identities | 79 |
| `MERGED_STANDARD` | 56 |
| `INTERNALIZED` | 10 |
| `CONCRETE_MOVED` | 8 |
| `DELETED` | 5 |
| deleted integration-test paths | 67 |
| `REPLACED` test paths | 60 |
| `PURE_REMOVED_API_IDENTITY` test paths | 7 |

The 79 identities include every Task 6 manifest path and 11 additional production identities found by the BASE scan. The table is exact: duplicate, missing or extra rows fail.

## 4. Source deletion and canonical ownership

The diff retires 149 old tracked paths: 148 are deletions and Git recognizes `physical_connection_info.rs -> connection_info.rs` as one 51%-similar rename.

- 82 production/implementation old paths;
- 67 integration-test files covered by the disposition table.

Production deletions are:

| area | retired old paths | disposition |
| --- | ---: | --- |
| `druid-rdbc/src/spi` | 24 | entire retired public module and trait tree deleted |
| `druid/src/spi` | 17 | facade/resource re-export shell deleted |
| Druid core compatibility files | 29 | 28 aliases/old duplicate objects deleted; `PhysicalConnectionInfo` concretely renamed to `ConnectionInfo` |
| `druid/src/rdbc.rs` | 1 | retired facade deleted |
| `druid/src/pool/pool_validation_factory.rs` | 1 | old compatibility factory deleted |
| wrapper private legacy helpers | 10 | per-family duplicate prepared/metadata helpers and old parameter resource state deleted |

Canonical replacements include:

- `druid_rdbc::{connection,driver,statement,result_set,metadata,wrapper,resource,extensions}`;
- Druid-private `ConnectionInfo`, `ExternalConnectionLease`, driver extension registry/descriptor and concrete resource policy objects under `druid::core`;
- wrapper-private common standard statement, prepared statement, result set and database metadata helpers;
- canonical SQLx actor validation and transaction cores shared by direct, bb8 and deadpool paths.

Empty `druid-rdbc/src/spi` and `druid/src/spi` directories were removed after their tracked files were deleted.

## 5. Behavior preserved or restored

During full regressions, three stale or missing cutover behaviors were exposed and closed with exact RED/GREEN tests:

1. `CommonDataSource::parent_logger` test expected the deleted `druid::rdbc` namespace; the canonical contract and test now use `druid::sql`.
2. SQLx `ResultSet::was_null` now preserves the last successful getter state when a later closed-cursor getter fails, matching the retained wrapper test.
3. SQLx canonical `is_valid`, auto-commit, commit and rollback now share raw/actor cores. bb8 and deadpool prove ping validation, reset-on-close, timeout, clean recycle and dirty-drop discard.

Task 5 lifecycle/filter/stat/cache/registry behavior remains green:

- logical lifecycle/cache: 18 tests;
- raw logical ownership: 5 tests;
- canonical filter/stat/wall/multi-result: 21 tests;
- metadata ownership/races: 3 tests;
- resource registry unit contracts: 10 tests through the Druid verifier;
- bb8/deadpool canonical external pools: 3 + 3 tests.

## 6. API evidence

`cargo-public-api 0.52.0` and the installed pinned nightly were available; no tool installation was needed.

| snapshot | lines | SHA-256 | retired surface |
| --- | ---: | --- | --- |
| `api/druid-rdbc-spi.transitional.txt` | transitional | `5a75fc52f1a310859d5723e4f0f6358643ece8f34670b6afedcd676f4fd3aafc` | 16 unique `Physical*` names; 1,279 `spi` lines |
| `api/druid-rdbc.single-standard.after.txt` | 1,388 | `e91be5670b87ed25d865e69c3bef11a97b9ab73552529a7d061b28b4f2f5fab1` | zero `Physical*`; zero `druid_rdbc::spi` |

The live public API generated during the final no-Physical verifier is byte-identical to the checked snapshot.

## 7. Manifest and evidence hashes

| artifact | BASE SHA-256 | final SHA-256 | final rows |
| --- | --- | --- | ---: |
| physical standardization | `523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1` | `c85784c98d065e6c9bafe5515eeaab4f41a6a8bb9aa5d568a606882f3bcaf62d` | 73 |
| adapter implementations | `c2af3627c426a2badfaf24a6447e4d53dd3c0d15cc76f8536a546cb758f8467e` | `90e34fafb5a234a44c6f6f5ba0cdd5a17605a87c0036c5bca63d0e88555061b2` | 31 |
| production consumers | `514dd501caa6ffec8a0cb262d436deca6c029ef4b3e0f9eba0d52b220571074d` | `c31e32bd04df7d92502f7c34d580f9b27ad5fa0751c89149f10bc54dc901dafa` | 3,647 |
| runtime traces | `106323e17904d377172986ce7464a54de60b4bb575e5f67e333e37f16188a3ec` | `bb63308865daf2e1d528e8e415e9438545919bf4752e300710c219a978bf344a` | 31 |
| compiler coverage | `f7b3d4b429fe9e5c4bfe954082f0518d8e78635f6f7cc21bde1ffcd40f2307c9` | `a2b06227c716c068d577582995bb6627756b8a5aef7c59f7b2d517278b13d7fe` | 7 × 15 |

Additional hashes:

- semantic live TSV: `f2e1d929af5c90df3bbeecb6416e4470b820105fd9f23cbbeed3246f95d9ed84`;
- normalized semantic digest: `5d99b57378649dbe16378f9e3992cfed46058bdb12cf138847006dce430cdbd9`;
- final 105-row BASE-to-final compiler identity/source map: `6303bca40bd16bc69be623c43aec5fe2ae299ab39df9448e9dbbf4407dffb38f`;
- migration document: `354dfe6bf4f9636c9db856fa3f15ce4afaf56093105f62b805350416dc92387a`;
- `Cargo.lock`: `f9b99b85810ceb01748dc5f9dbf58ff715f70838b74768ed4f45f5989fc3e089`.

## 8. Task 4 coverage rebuild

Seven fresh isolated reports were generated after the final Rust source stopped changing. Every report proved the exact test identity, family factory, family executor, 11 typed stage anchors and post-validation success function. The manifest was then regenerated atomically and the repository verifier reran all seven families from fresh isolated profiles.

Final verifier output:

```text
WRAPPER_STANDARD_ADAPTERS_LIVE=31
WRAPPER_STANDARD_CONSUMERS_CLOSED=21
FAMILY_COMMON_PREPARED_CHAINS_LIVE=6
SQLX_OWNED_ACTOR_LIVE=1
RUNTIME_TRACE_FAMILIES_LIVE=7
RUNTIME_TRACE_PROTOCOL=task4-runtime-trace-v1
COMPILER_COVERAGE_FAMILIES_LIVE=7
COMPILER_COVERAGE_PROTOCOL=task4-compiler-coverage-v2
NONBEHAVIOR_IDENTITY_ROWS=2
```

The Task 5-only frozen-manifest/RBDC source-hash exception was removed. The Druid verifier now authenticates the regenerated Task 6 coverage/runtime hashes, validates all current source hashes and field-exact family bindings, and then runs the retained Task 5 behavior suites.

Final isolated report SHA-256 values:

| family | report SHA-256 |
| --- | --- |
| DUCKDB | `34f6419ca659c4ca688cb36309362ce8cd766ff0c92e23053b0481e465986be6` |
| HTTP_SQL | `ef178d6daf38fad6e760c5fd6e25641304dc235cf47ad0cfe22dc6dec4270515` |
| JDBC_AGENT | `582999446958148db2c972e134b5e1ecb4433ffdfd472f0498c303df8cc1bb42` |
| LIBSQL | `51891c75fb8c4065a07701bf0a8db3d7a9392cd713a5cf652eec82c92ce39c75` |
| RBDC | `92824dcb255d2147c310c1b305d5a4fb7a36b606ea89148af5b81b4d73c8259b` |
| SQLX | `9db378a4d9feed85338710e35c49eac54c3ba92982a51fd0df608f12ed1c8bb9` |
| TOASTY | `dea667a7be7e54e18cd703f42fb2f6fe529f80a5ddce016dadb1c0f64a8a3ab7` |

## 9. Dependency cleanup

Direct `druid-rdbc` dependencies removed:

- `jiff`;
- `num-bigint`;
- `parking_lot`;
- `serde_json`;
- `thiserror`.

Retained deliberately:

- `tokio` with `sync` for canonical Task 7 resource synchronization;
- `bitflags` for canonical resource operation flags;
- standard value/URL/time dependencies used by the canonical trait tree.

`Cargo.lock` removes only those five direct edges from the `druid-rdbc` package record. The dependency-boundary verifier and default/all-feature forward/inverse Cargo trees pass. Tokio may still bring `parking_lot` transitively; it is not a direct `druid-rdbc` dependency.

## 10. Verification matrix

| gate | result |
| --- | --- |
| no-Physical verifier self-tests | PASS, 6/6 |
| no-Physical live/public/rustdoc/migration/test-disposition verifier | PASS |
| external canonical/legacy public-surface test | PASS, 1/1 |
| Task 3 purity self-tests and final purity | PASS, 103/103 + `STANDARD_FILES=33` |
| denominator historical/final self-tests and final denominator | PASS, 43/43 + `73 / 31 / 3647` |
| wrapper verifier self-tests | PASS, 38/38 |
| Druid verifier self-tests | PASS, 26/26 |
| dependency boundary | PASS |
| Task 5 Druid verifier | PASS, `180 / CLOSED75 / 108`, six standard impls |
| Task 4 exact runtime/compiler coverage | PASS, 7/7 × 15 |
| semantic verifier self-tests and logical inventory | PASS, 18/18 + `52/17/0` |
| `druid-rdbc` full all-target/all-feature tests | PASS |
| `druid` full all-target/all-feature tests | PASS |
| `druid-wrapper` full all-target/all-feature tests | PASS |
| workspace all-target/all-feature check | PASS |
| strict three-crate Clippy `-D warnings` | PASS |
| strict three-crate rustdoc and doctests | PASS, doctests `0/1/0` |
| rustfmt | PASS |
| Cargo default/all-feature forward/inverse trees | PASS |

An additional broad `cargo test --workspace --all-targets --all-features` was executed. Every Task 6/RDBC/Druid/wrapper target passed, then pre-existing `druid-admin` binary startup/bind/shutdown tests were flaky under concurrent workspace execution. A serial admin rerun passed 14/15; the remaining gRPC sibling-bind test passed 1/1 when rerun exactly. No admin runtime behavior was changed for this out-of-scope timing/port risk.

## 11. External boundaries

The following remain **NOT_RUN** and are not represented as deployment success:

- real external RBDC service/database;
- real HTTP SQL service;
- real libSQL remote service;
- real JDBC Agent H2/HSQLDB service;
- publish, push or deployment.

Deterministic local transports, SQLite execution and exact compiler coverage prove the in-repository contracts only.

## 12. Remaining risk and handoff

- This is an intentional breaking API cutover; consumers using `druid_rdbc::spi` or `Physical*` must migrate using the checked table.
- Task 7 resource hardening remains pending by design; its canonical resource surface was preserved.
- The unrelated `druid-admin` concurrent binary-test flake remains outside Task 6 and is recorded rather than hidden.
- Independent review is still required before Task 6 can be marked complete and Task 7 starts.

The independent commit SHA is recorded in the final handoff rather than inside this report because a commit cannot contain its own object ID.
