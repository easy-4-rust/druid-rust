# Task 6 implementation report — single-standard breaking cutover

Status: **FIX_ROUND_1_IN_PROGRESS**

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

## 13. Independent review correction — FIX_REQUIRED

Independent review of `5afe860..6e2b108` returned **FIX_REQUIRED** with `3 Critical + 2 Important`. The prior implementation-complete wording is superseded by this correction.

- **C1 — canonical method/behavior loss:** retired public traits contain hundreds of direct methods absent from the canonical traits. SQLx savepoint, read-only, warnings, metadata/type mapping, generic execute/batch and related behavior were deleted rather than merged or explicitly dispositioned.
- **C2 — deleted-test denominator is not behavioral:** 67 deleted files contain 1,283 BASE test functions, but the file-level table names only 33 replacement functions. Six of seven `PURE_REMOVED_API_IDENTITY` files contain 113 behavior tests.
- **C3 — external-pool discard does not propagate:** dirty Druid leases return transaction-active SQLx connections to bb8/deadpool. External reproduction observed `auto_commit=false` after immediate reacquisition.
- **I1 — migration authorities disagree:** 38/67 overlapping paths disagree across the migration document and standardization TSV; Task 7 targets are prematurely closed, exact module paths are not proven, and the claimed 105-row compiler map was not committed.
- **I2 — unlisted public/config/wire breaks:** Druid/wrapper/admin public methods, configuration aliases and serialized keys changed outside a complete checked breaking allowlist; only `druid-rdbc` had a public API snapshot.

Fix round 1 owns C3 plus I1/I2 integration. Controller-assigned workers separately own the canonical method-level parity matrix/implementation and the exact 1,283-function test ledger/migrated tests. Task 7 remains blocked until all five findings are integrated, full gates rerun and independent re-review returns clear.

## 14. Fix round 1 integration evidence — IN PROGRESS

This section supersedes the stale C1-C3/I1-I2 claims above, but does not yet restore a completion
verdict because the exact C2 executable ledger and final Task 4 rebuild are still open.

### C1 canonical method and implementation parity

- The checked BASE method denominator is 479 rows: the seven retired Physical traits contribute
  460 direct methods and the related `ConnectionExt` family contributes 19.
- Final dispositions are `215 MERGED_EXACT / 262 MERGED_TYPED / 2 INTERNALIZED`, with no pending
  method and no unapproved method-level break; the TSV SHA-256 is
  `432ef376e106b7fb98e3ea09dbb4e0bfcf46fd0391688bf829c010f7d3a7d360`.
- Canonical SQLx/Druid implementations and focused Prepared, Callable, ResultSet, metadata,
  filter/batch and external-consumer matrices are GREEN. Task 3 final purity is independently
  GREEN at `104/104` self-tests and `STANDARD_FILES=42`.

### C3 external-pool discard propagation

- A private checkout-scoped poison control now reaches bb8 `has_broken` and deadpool `recycle`
  without adding another SQL trait hierarchy.
- Both size-one pools cover clean reuse plus dirty logical drop, fatal sorter disposition and recycle
  error. Every discard case immediately reacquires a new clean connection, proves
  `auto_commit=true`, proves no uncommitted row crossed borrowers and observes
  `create_count=2 / destroy_count=1`.
- Fresh integrated result: bb8 `5/5 PASS`, deadpool `5/5 PASS`.

### I1 single migration authority

| artifact | current evidence |
| --- | --- |
| `task6-migration-authority.tsv` | 79 rows, SHA `e516076f2574c64cd62cc69d3ae99d6755773a370094e65ba33d5d3f2581705c` |
| method migration | 479 rows = Physical 460 + ConnectionExt 19; SHA `432ef376e106b7fb98e3ea09dbb4e0bfcf46fd0391688bf829c010f7d3a7d360` |
| exact deleted-test ledger | 1,283 rows / 65 test-bearing files plus two zero-test dispositions; current structural gate is 1,160 exact + 123 parameterized, execution still in progress |
| physical standardization | 73 rows, SHA `2a00a21bf49d7136696272fdfbc49a491fc8798ae171cdf02f7ec2bfc4f374b6` |
| Task 7 handoff | authority `23 PENDING`; physical history `29 PENDING`; `RowSetResultSet` is closed by live `BufferedRowSet` plus 73-case evidence while the distinct `RdbcResultSet` forwarding identity remains pending |
| compiler identity map | committed 105-row canonical TSV; provisional SHA `ae6fe28be5f0197a7e27cc78520689edf0e1577dee4d9ea0b488efdbcedcd590` pending final seven-family regeneration |

The 67 overlapping authority/standardization rows now agree field-exactly on disposition, target and
task. Private targets resolve through their exact declared module rather than a same-named leaf
elsewhere. The Markdown migration table and honest 67-file deleted-test index are generated from the
TSV authorities.

### I2 public/config/wire contract

| evidence | current result |
| --- | --- |
| `druid-rdbc` current API | 2,631 lines, SHA `2a1d1ef8b89bf7f3a7963c199da5a1694985a246669e8a6283acb6622ef9bdbf` |
| `druid` current API | 14,654 lines, SHA `1ae563cd0b7f3f5ec4b8142cbc58977e1dafca5f4ed24997f2e436f6513b6fcc` |
| `druid-wrapper` BASE/current API | 1,986 / 1,067 lines; BASE SHA `417204370b4e0fe0c03c88385e322405be53d965507de1407ce6b5fccc34fbe6`; current SHA `dd9af9bc1314023bcebc237444b88ada0e949977ab31348faed7a3ee56ae19f0` |
| exact allowlist | 128 rows, SHA `43431f756fceac8b8f50096c97936a22889f6ef2256b736ce24685baf9f71015` |
| retained aliases | five exact deprecated API paths for `physical_connection_timeout`, `phy_timeout`, HA timeout and the config field |
| focused behavior | Driver `2/2`, config alias `2/2`, admin JSON `1/1`, external three-path consumer PASS |

The 128 rows comprise 105 exact Druid/wrapper API paths, 17 restored Driver/DriverProperty contract
rows, three admin Rust fields and three serialized JSON keys. All three current API snapshots have
zero retired Pascal-case names. The approved admin
wire break emits only `DriverConnectCount`, `DriverCloseCount` and `DriverConnectErrorCount`; its
golden test proves both values and absence of the retired keys.

### Remaining before completion

- C2 replacement targets are not yet executable as a complete set; static 1,283/1,283 binding is
  necessary but not sufficient. The latest generator pass reduced compilation to 74 diagnostics in
  six typed-fixture files; ResultSet 28-case, BufferedRowSet 73-case and Prepared 22-case matrices
  are already GREEN, with no newly discovered production gap.
- Task 4 compiler coverage correctly reports stale source hashes. All seven fresh reports must be
  regenerated atomically, the 105-row map refreshed, and then an independent 7/7 rerun must pass.
- Final full tests, strict Clippy/docs/fmt, dependency DAG/trees, hashes, stage and commit remain
  pending; no completion claim is made here.

## 15. Fix round 1 final integration correction — IMPLEMENTED, PENDING REVIEW

This section supersedes the **IN PROGRESS** status and provisional hashes in section 14. It does not
supersede the independent-review requirement: Task 6 remains pending a fresh review of the new
commit before Task 7 may start.

### C1 — canonical method and behavior parity closed

- The exact BASE denominator is 479 methods: `ConnectionExt=19`, `PhysicalConnection=52`,
  `PhysicalStatement=30`, `PhysicalPreparedStatement=32`, `PhysicalCallableStatement=32`,
  `PhysicalResultSet=120`, `PhysicalDatabaseMetaData=173` and
  `PhysicalResultSetMetaData=21`.
- Final dispositions are `215 MERGED_EXACT / 262 MERGED_TYPED / 2 INTERNALIZED`, with zero pending
  or approved-break rows. The canonical method TSV SHA-256 is
  `35e294a3562b88807650debd7fdf6b9f9a423d3b8b536c39406270029a48bd68`.
- Exact evidence is 479 rows / 295 unique selectors, comprising 328 C2-derived rows and 151 unique
  fallback cases. Executed evidence covered 19 targets / 86 functions / 295 selectors. Its SHA-256
  is `e2fa8ed38bcea66741596e02f775e99476f1440fa9302fc465e9b6456a591d56`.
- The verifier now accepts crate-private `src#module::tests::function` evidence only when its full
  module path matches the exact source module. Leaf-only or wrong-module matches reject.
- Canonical PreparedInputParameter, ConnectionExt, Statement/Prepared/Callable ordered results,
  ResultSet navigation/update/stream behavior, metadata, warnings, generic execute/batch and
  SQLx/Toasty transaction semantics are restored on the one standard trait tree.

### C2 — exact 1,283-function test handoff closed

- The authoritative ledger contains 1,283 unique BASE identities across 65 test-bearing files:
  `1,065 MIGRATED_EXACT / 218 MERGED_PARAMETERIZED / 0 removal`. Two additional zero-test paths are
  dispositioned separately, producing the honest 67-file index.
- Final authoritative execution is `executed=yes`: 67 source files, 81 current targets, 1,073
  target test functions and 218 parameter case digests. The ledger SHA-256 is
  `b66867b565cdb649129eaa872d1bafe267674056c5b2fc07554e5d6f9397df91`.
- The 11,865-row exact BASE method-call reverse index SHA-256 is
  `c72e2c41cc7caa3a8b1aa83d1c26e9f4faeb242ecadf0e113005acbc6088cca4`.
- The deleted-test index in `physical-to-standard-migration.md` is renderer-derived from the final
  ledger, including the approved split of the pooled-prepared target. No unique runtime behavior is
  deleted merely because its original test imported a retired name.

### C3 — pool-native poison and one-shot owner disposition closed

- Private `OwnerDispositionControl` separates synchronous discard notification from later real
  holder transfer. Fatal/dirty/recycle-error paths notify exactly once, invalidate all later
  operations, and transfer/close the holder only on logical close or Drop without duplicate pool
  accounting.
- Native DruidPool accounting immediately observes `active=0 / close=1 / discard=1 / destroy=1`;
  the raw close still occurs exactly once during later holder transfer. A clean replacement is
  reusable.
- bb8 and deadpool manager poison is shared with the logical lease. Size-one tests cover clean
  reuse, transaction-dirty Drop, fatal sorter and recycle error; every discard reacquires a new
  `auto_commit=true` connection with no uncommitted rows and exact create/destroy/rejection counts.
- Fresh focused evidence: bb8 `5/5`, deadpool `5/5`, native owner disposition `1/1`, warning
  semantics `6/6`, pool coverage `37/37`, maintenance differential `10/10` and maintenance
  semantics `14/14`.

### I1 — one authority, exact Task 7 state and compiler map closed

| artifact | final evidence |
| --- | --- |
| migration authority | 79 rows; `44 MERGED_STANDARD / 9 INTERNALIZED / 22 CONCRETE_MOVED / 4 DELETED`; `56 TASK6_CLOSED / 23 TASK7_PENDING`; SHA `e516076f2574c64cd62cc69d3ae99d6755773a370094e65ba33d5d3f2581705c` |
| physical standardization | 73 rows; SHA `2a00a21bf49d7136696272fdfbc49a491fc8798ae171cdf02f7ec2bfc4f374b6` |
| adapter implementations | 31 rows; SHA `90e34fafb5a234a44c6f6f5ba0cdd5a17605a87c0036c5bca63d0e88555061b2` |
| production consumers | 3,647 rows; SHA `c31e32bd04df7d92502f7c34d580f9b27ad5fa0751c89149f10bc54dc901dafa` |
| migration Markdown | renderer-derived 79 + 67 file rows; SHA `7e1f172ca6901622580d4ab4557b3ac14a74dbce036d87d42bbb49043cf90843` |
| compiler coverage manifest | seven families × 15 exact identities; SHA `e83862418e32b4834acdbbda14c6060a29be010ffc971553c83f01e1748c6d77` |
| BASE→Task6 compiler identity map | 105 exact rows, protocol `task4-compiler-coverage-v2`; SHA `ac39d5b95e9417287b5b9530f3c7595a50a4b9221d5fee75bac8581a81432743` |

Task 7 resource rows remain pending unless their target/evidence exists. `RowSetResultSet` is closed
by live `BufferedRowSet` behavior, while the distinct `RdbcResultSet` forwarding identity remains
pending. Exact module resolution is required; a same-named leaf elsewhere cannot satisfy a target.

The final coverage rebuild added a cheap 105/105 source/function preflight, exact SQLx and Toasty
executor call-chain checks, and a Rust-v0 top-level test-identity selector that rejects nested async
closure/coroutine symbols. Trace callsite multiplicities are pinned to current mutually exclusive
branches as `StatementUpdate=2 / PreparedQuery=2 / Toasty DatabaseMetadata=2`; every emitted runtime
trace still contains each of the 11 stages exactly once.

Final isolated report SHA-256 values are:

| family | final report SHA-256 |
| --- | --- |
| DUCKDB | `ec5d7bc215fd4d9ce3a063ca57fdc08eb19de4a4e0222f34bc2b14fb51049201` |
| HTTP_SQL | `64c213cc6e5391db23fd64fdfd1fa9dfddf823584bf20206de2a82f5015de6f6` |
| JDBC_AGENT | `1939adba70cc343ce98811d397d5ed48527d6f8097a1deee405fd7367b7461e5` |
| LIBSQL | `539a12b8429ebe4b8278b33d3031ccf6fbc2eff42ab11a7bc10a068adaced9b7` |
| RBDC | `ae5d107c99337a5950a16c873c77101cdcfaa1a31237b23f057aff52de82f5ea` |
| SQLX | `216a4d3667abd9d8e046c753a4ecf0aeaa9a3f3ca1a67c5027056bab4d58327b` |
| TOASTY | `2793af47b2e1c039506cb1ef4f1e2cd39604938f57ae338cc380c6df763ea334` |

After atomic regeneration, an independent isolated verifier rerun passed all seven families and all
15 identities per family with no source, hash or compiler-identity drift.

### I2 — public/config/wire contract closed

`cargo-public-api 0.52.0` and pinned `nightly-2026-08-07` generated all snapshots; no tool was
installed during Task 6.

| snapshot | lines | final SHA-256 | retired names |
| --- | ---: | --- | ---: |
| `druid-rdbc.single-standard.after.txt` | 2,631 | `2a1d1ef8b89bf7f3a7963c199da5a1694985a246669e8a6283acb6622ef9bdbf` | 0 |
| `druid.single-standard.after.txt` | 14,733 | `a03588106051988e338a5d3986832c2e08a5cb004bdb7f817ddadeca6dff17e5` | 0 |
| `druid-wrapper.single-standard.after.txt` | 1,107 | `48c1c403594346342322ff5bc629c3c65effb107fd3b7fd5b776df8042555f3b` | 0 |

The checked public/config/wire allowlist remains exactly 128 rows, SHA
`43431f756fceac8b8f50096c97936a22889f6ef2256b736ce24685baf9f71015`. Five reviewed lowercase
timeout compatibility aliases remain explicitly deprecated; no retired Pascal-case production/API
identity is retained. Driver/DriverProperty, admin JSON/config and external consumer golden tests are
live and GREEN.

### Final zero counts and verification

The final aggregate no-Physical verifier reported:

```text
PRODUCTION_RUST_FILES_SCANNED=623
PUBLIC_PHYSICAL_NAMES=0
PRODUCTION_PHYSICAL_NAMES=0
RUSTDOC_PHYSICAL_NAMES=0
DRUID_RDBC_SPI_MODULES=0
MIGRATION_IDENTITIES_CLOSED=79
NEWLY_DISCOVERED_IDENTITIES_CLOSED=11
TRANSITIONAL_CONSUMER_IDENTITIES_CLOSED=75
DELETED_TEST_FUNCTION_MIGRATIONS=1283
PUBLIC_CONFIG_WIRE_BREAKS_DISPOSITIONED=128
ACTIVE_TRANSITIONAL_CONSUMERS=0
```

| final gate | result |
| --- | --- |
| no-Physical self/live/external API/rustdoc | PASS, self `7/7`, live counts above, external surface `1/1` |
| method verifier | PASS, self `10/10`, 479 rows and `executed=yes` evidence |
| deleted-test verifier | PASS, self `11/11`, `1,283/1,283 executed=yes` |
| Task 3 purity | PASS, self `104/104`, `STANDARD_FILES=42` |
| Druid verifier | PASS, self `27/27`, `180 / CLOSED75 / PENDING108`, six standard impls |
| denominator | PASS, self `43/43`, `73 / 31 / 3647` |
| migration authority/public contract | PASS, `79 / pending23` and allowlist `128` |
| Task 4 compiler coverage | PASS, verifier self `41/41`, final independent `7/7 × 15` |
| dependency boundary and Cargo trees | PASS; forward default/all-feature `128/128` lines, workspace inverse `14/15` lines |
| three core crates all-feature tests | PASS (`druid-rdbc`, `druid`, `druid-wrapper`) |
| workspace all-target/all-feature check | PASS |
| strict workspace all-target/all-feature Clippy | PASS with `-D warnings` |
| strict three-core-crate rustdoc/doctests | PASS; doctests `0/1/0` |
| rustfmt / diff check | PASS |

Dependency cleanup from the original Task 6 cutover remains unchanged: the five obsolete direct
`druid-rdbc` edges listed in section 9 stay removed, Task 7-required synchronization/resource
dependencies stay retained, and fix round 1 does not modify Cargo manifests or `Cargo.lock`.

The attempted **workspace-wide** strict rustdoc gate passed the three Task 6 crates and then failed
on two unrelated existing `druid-metrics` links: `DataSourceMonitorable` in
`crates/druid-metrics/src/lib.rs:4` and `DruidMetricsConfig` in
`crates/druid-metrics/src/error.rs:1`. Per controller scope ruling, Task 6 did not edit those files;
the exact three-core-crate rustdoc/doctest gate is GREEN.

### Remaining boundaries and risk

- The cutover is intentionally breaking for consumers of `druid_rdbc::spi` or any `Physical*`
  public name; the checked migration authority is the required upgrade map.
- Task 7 resource hardening remains pending by design and was not pulled into this fix round.
- Real external RBDC, HTTP SQL, libSQL remote and JDBC Agent H2/HSQLDB services remain **NOT_RUN**.
- Publish, push and deployment remain **NOT_RUN**.
- The unrelated workspace-wide `druid-metrics` rustdoc links above remain outside Task 6 scope.
- A fresh independent review of the fix commit is still required; no Task 6 completion or Task 7
  start is claimed before review clear.
