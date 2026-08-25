# P04 Task 4 report — wrapper integrations directly implement standard

## 1. Scope and baseline

- BASE / starting HEAD: `48f6af43ad74c915704f1632c7ffd3f4705a219f`。
- Branch: authorized current `main`; no worktree, branch switch, reset, amend, push, Cargo topology change, or `druid-admin` edit。
- Spec source: `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md` / ADR-RDBC-006。
- Task source: `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-4-brief.md`。
- Toolchain / MSRV: Rust `1.97.1` / `1.95` / edition 2021。
- CodeGraph: repository has no `.codegraph/` directory。
- Task 3 prerequisite: recovery `REVIEW CLEAR`; purity `103/103 + live29`。
- Task 2 frozen semantic denominator: physical `73`, adapter `55`, consumer `3559`。

No Task 5 Druid holder/pool/proxy cutover and no Task 6 Physical deletion is included。

## 2. Scope-extension rulings

The parent coordinator explicitly approved these minimum prerequisites after fresh RED evidence:

1. Extend `verify_jdbc42_semantic_inventory.py` and its self-test with fail-closed
   `--phase adapter-live`; do not add `logical-live`。
2. Extend `verify_rdbc_standardization_denominator.py` and its self-test so only
   `P04_TASK_4_ADAPTER_CUTOVER` wrapper rows accept `STANDARD_IMPL_LIVE` /
   `STANDARD_REFERENCE_LIVE`; every source-derived field still compares exactly。
3. Mechanically change only one old same-crate test expression in
   `prepared_statement_semantics_test.rs` from a redundant closure to
   `BatchUpdateCounts::large`, closing wrapper all-target Clippy。

No manifest or dependency change was required。

## 3. TDD RED evidence

| Gate | Fresh RED | Cause |
| --- | --- | --- |
| wrapper verifier self-tests | `5/5 PASS` | positive control for the verifier itself |
| live wrapper verifier | FAIL at first DuckDB row | canonical `Connection` impl absent; adapter only implemented `PhysicalConnection` |
| standard adapter conformance | compile FAIL, 14 `E0277` | SQLx/RBDC/Toasty connection/factory/prepared/metadata and two data sources lacked canonical traits |
| statement ownership | compile FAIL, 2 `E0277` | `SqlxConnectionAdapter: Connection` absent |
| metadata ownership | compile FAIL, 3 `E0277` | owned standard connection/metadata path absent |
| semantic adapter phase | CLI exit 2 | `--phase adapter-live` was not implemented |
| Task 2 live denominator | FAIL | `STANDARD_IMPL_LIVE` was an unknown state |
| wrapper all-target Clippy | FAIL | pre-existing same-crate redundant closure at `prepared_statement_semantics_test.rs:1851` |

All RED failures were the intended missing behavior/schema, not syntax or environment failures。

## 4. GREEN implementation

### 4.1 Seven adapter families

The 31 `P04_TASK_4_ADAPTER_CUTOVER` production adapter rows now have direct canonical impls and
`STANDARD_IMPL_LIVE` state:

| Family | Connection | Factory | Prepared | DatabaseMetaData | Extra |
| --- | --- | --- | --- | --- | --- |
| SQLx | direct | direct | direct | direct owned live | direct `ResultSet` |
| RBDC | direct | direct | direct | direct owned/Cow | — |
| Toasty | direct | direct | direct | direct owned/Cow | — |
| DuckDB | direct | direct | direct | direct owned Arc | — |
| HTTP SQL | direct | direct | direct | direct owned snapshot | — |
| libSQL | direct | direct | direct | direct owned snapshot | — |
| JDBC Agent | direct | direct | direct | direct cloned snapshot | — |

`DruidRdbcDataSource` and `DynamicRdbcDataSource` directly implement canonical `Wrapper` identity。
The transitional Physical impls remain only for Task 5 migration。

### 4.2 One implementation core

- Every standard factory and transitional factory delegates the same `create_adapter_core`。
- SQLx standard update and Physical update delegate `execute_update_core`; query handles delegate
  `fetch_rows_with_labels`。
- SQLx standard/Physical ResultSet delegate `next_core`, `value_core`, `close_core`,
  `is_closed_core`, and `was_null_core`。
- Metadata standard/Physical impls use the same product-name core。
- Canonical prepared parameters use the shared `PreparedParameterMaterializer` and
  `PreparedParameterState`; execution/default/error bodies were not copied into a second hierarchy。
- Fresh source identity comparison against BASE: `CONSUMER_IDENTITY_ADDED=0`,
  `CONSUMER_IDENTITY_REMOVED=0` after removing every standard-to-Physical reverse delegation。

### 4.3 SQLx owned actor / lease

`SqlxConnectionActor` owns the raw `SqlxConnectionAdapter` behind `tokio::sync::Mutex`。

- `Statement` and `PreparedStatement` own `Arc<SqlxConnectionActor>` after the create-call borrow ends。
- `ResultSet` keeps an actor lease until `close`; it remains mutable and uniquely boxed。
- live `SqlxDatabaseMetaData<'static>` owns the same actor and returns owned ResultSet handles。
- `ResultSetMetaDataSnapshot` and `PreparedParameterMetaDataSnapshot` are immutable `Arc` snapshots
  without connection borrows。
- No lock guard is exposed; actor operations are serialized through one owner and no unsafe/manual
  `Send`/`Sync` implementation was introduced。

## 5. Live inventories and hashes

### 5.1 Task 4 transitions

- Adapter rows LIVE: `31/31`。
- Task 4 wrapper consumer rows LIVE: `21/21`。
- Semantic adapter-live objects: Connection, Statement, PreparedStatement, ResultSet,
  DatabaseMetaData, ResultSetMetaData, ParameterMetaData, Wrapper。
- Resource/RowSet objects assigned to Task 7 remain honestly `PLANNED`。
- Logical implementation fields remain `PLANNED`; Task 5 was not claimed。

### 5.2 Task 2 denominator

Fresh exact source scan:

```text
PHYSICAL_ROWS=73
ADAPTER_ROWS=55
CONSUMER_ROWS=3559
CONSUMER_SHA256=63dab31f759598d8323ced636d831ead68d9502a1697434686dd0f75f8cefd00
```

BASE consumer raw SHA was
`b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59`。
The raw hash changed because it includes token coordinates and Task 4 added owned-handle fields/core
code. It is not reported as unchanged. The denominator count and coordinate-independent full
source identity multiset are unchanged (`added=0`, `removed=0`)。

Semantic phase hashes:

```text
JAVA_SQL=52
JAVAX_SQL=17
UNKNOWN=0
SEMANTIC_SHA256=5d99b57378649dbe16378f9e3992cfed46058bdb12cf138847006dce430cdbd9
LIVE_SEMANTIC_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

The first semantic hash is the phase-normalized Task 2/3 denominator; the second is the canonical
Task 4 LIVE TSV bytes。

## 6. Configured / NOT_RUN matrix

| Family | Build/conformance | Live driver execution in this checkout | Status |
| --- | --- | --- | --- |
| SQLx SQLite | default + all features | real `sqlite::memory:` actor/query/update/metadata | PASS |
| Toasty SQLite | default + all features | real `sqlite::memory:` physical regression | PASS |
| RBDC | default + all features | deterministic fake RBDC driver regression | PASS; external DB NOT_RUN |
| DuckDB native | all features | real `duckdb::memory:` including cancel/timeout/restart | PASS |
| HTTP SQL | all features | no endpoint/credentials configured | conformance PASS; live NOT_RUN |
| libSQL native | all features | no remote URL/token configured | conformance PASS; live NOT_RUN |
| JDBC Agent | all features | `DRUID_JDBC_AGENT_JAR`, H2/HSQLDB jars absent | protocol/conformance PASS; live NOT_RUN |

Environment audit also found `DRUID_DATABASE_PROFILE`, `DRUID_DATABASE_URL`,
`DRUID_REQUIRE_DATABASE_PROFILE_CONTRACT`, `DRUID_JDBC_AGENT_JAR`,
`DRUID_JDBC_DRIVER_JAR`, and `DRUID_HSQLDB_DRIVER_JAR` unconfigured。No skipped live driver is
reported PASS。

## 7. Final verification evidence

### Fail-closed / phase gates

- `python3 scripts/test_verify_wrapper_standard_impls.py` — `5/5 PASS`。
- `python3 scripts/verify_wrapper_standard_impls.py` — `31/21`, actor live。
- `python3 scripts/test_verify_jdbc42_semantic_inventory.py` — `16/16 PASS`。
- `python3 scripts/verify_jdbc42_semantic_inventory.py --phase adapter-live` — `52/17/0 PASS`。
- `python3 scripts/test_verify_rdbc_standardization_denominator.py` — `39/39 PASS`。
- `python3 scripts/verify_rdbc_standardization_denominator.py` — `73/55/3559 PASS`。
- `python3 scripts/test_verify_rdbc_single_standard_purity.py` — `103/103 PASS`。
- `python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional` — live `29 PASS`。

### Focused Rust gates

- standard conformance default `2/2`; all features `6/6`。
- statement ownership `2/2`。
- metadata ownership `2/2`。
- adapter boundary `4/4`。
- SQLx adapter `8/8`; RBDC `11/11`; Toasty `6/6`; DuckDB all-feature `9/9`。
- `cargo test -p druid-rdbc --all-targets --all-features` — `20/20 PASS`。
- `cargo test -p druid-rdbc --doc` — `1/1 PASS`。
- `cargo clippy -p druid-rdbc --all-targets --all-features -- -D warnings` — PASS。

### Broad Rust gates

- `cargo test -p druid-wrapper --tests` — all test binaries PASS。
- `cargo test -p druid-wrapper --all-features --tests` — all test binaries PASS。
- `cargo check -p druid-wrapper --all-features --all-targets` — PASS。
- `cargo check --workspace --all-targets --all-features` — PASS。
- `cargo clippy -p druid-wrapper --all-targets --all-features --no-deps -- -D warnings` — PASS
  after the explicitly authorized one-line existing-test fix。
- `cargo doc -p druid-wrapper --all-features --no-deps` — PASS。
- `cargo test -p druid-wrapper --doc --all-features` — PASS (`0` doctests)。
- `cargo fmt --all -- --check` — PASS。
- `git diff --check` — PASS。

## 8. Open risks / reviewer decisions

1. The exact broad dependency-aware Clippy command is **not GREEN** and is not reported GREEN:
   `cargo clippy -p druid-wrapper --all-targets --all-features -- -D warnings` fails in two
   unchanged Druid Task 5 files:
   - `crates/druid/src/core/druid_pooled_connection.rs:1479` (`useless_conversion`);
   - `crates/druid/src/core/result_set_filter_chain.rs:1309` (`useless_conversion`)。
   Per ruling these files were not modified; final reviewer must adjudicate this external baseline risk。
2. External HTTP SQL/libSQL/JDBC Agent and real RBDC database services were not configured; their
   live network/server behavior remains NOT_RUN。
3. `SqlxStreamingResultSet` remains eager native-row materialization (`fetch_all`), not a true async
   streaming cursor; Task 7 owns that hardening。
4. Transitional Physical impls/names intentionally remain for Task 5/6 and are not a second
   canonical hierarchy。

## 9. Completion state

Task 4 implementation and verification are complete pending independent review。Task 5 has not
started。

## 10. Independent review correction

Independent review of `48f6af4..8252a20` returned **FIX_REQUIRED** with
`1 Critical + 4 Important + 1 Minor` open：

- C1: compile-time direct impl/LIVE claims are not backed by canonical behavior for six non-SQLx
  families; 21 SqlText consumer rows still reference transitional `druid_rdbc::spi` objects;
  verifier accepts seven adversarial false positives。
- I1: SQLx close/open/capability/generated-keys contracts conflict。
- I2: byte-counting `?` is not driver placeholder metadata。
- I3: `was_null` does not reset after a successful non-null getter。
- I4: `TablesQuery` ignores catalog/schema/types and hard-codes SQLite SQL for every backend。
- M1: source-projection and checked-LIVE TSV hashes were mislabeled。

The previous completion wording is superseded。Task 5 is blocked until all load-bearing findings
receive strict RED/GREEN fixes and independent re-review returns clear。

## 11. Fix round 1/5 implementation (base `8252a20`)

This section supersedes Sections 5-9 for current counts, hashes, and finding disposition. It does
not change the independent review verdict: Task 5 remains blocked until this fix commit receives an
independent clear review.

### 11.1 Strict RED -> GREEN evidence

| Finding | Fresh RED on `8252a20` | Minimal GREEN implementation and permanent evidence |
| --- | --- | --- |
| C1 canonical behavior | Toasty standard DDL, RBDC setter, and DuckDB standard DDL returned typed `Unsupported`; seven adversarial verifier fixtures were accepted | Six non-SQLx factories now return wrapper-private owned Statement/Prepared/ResultSet handles bound to a family executor; standard and Physical paths call the same inherent/private execution cores without reverse delegation. Toasty SQLite, DuckDB native, and deterministic RBDC standard factory paths execute real query/update behavior. HTTP SQL, libSQL, and JDBC Agent reach their owned family core and return typed NOT_RUN errors in this unconfigured checkout. Verifier self-tests are `8/8`, including empty, split-core, standard-to-Physical, local-leaf, cfg-disabled, alias, and comment-only rejection. |
| I1 SQLx lifecycle | capabilities advertised transactions/savepoints while standard commit was unsupported; execute survived statement close; generated-key requests were ignored | transaction/savepoint capabilities are false until implemented; generated keys are typed unsupported. Setter/execute/metadata operations require an open actor; Statement/Prepared/ResultSet close takes its `Option<Arc<_>>` lease. Standard and Physical connection close share `close_connection_core`; close error propagates and can be retried, while successful close is exactly once. Connection-first, statement-first, double-close, and concurrent close/execute tests pass. |
| I2 placeholders | quoted/comment question marks produced three metadata parameters instead of one | SQLx preparation uses the driver's prepared statement parameter description; no byte-counting or cross-dialect parser remains. Configured SQLite covers quoted/comment `?` and `?1`; other drivers remain typed NOT_RUN unless actually configured. |
| I3 `wasNull` | NULL then non-NULL remained `true` | every successful getter stores `matches!(value, Value::Null)`; `next` resets, while closed/error paths preserve the last successful state. Permanent sequence/reset/error tests pass. |
| I4 tables | `table_types=Some([])` still returned a row | the actor routes by backend; unsupported backends fail typed. SQLite distinguishes catalog, schema, pattern, `None`, empty, TABLE, and VIEW filters. Permanent nonmatching and filter tests pass. |
| M1 hash labels | source projection and checked TSV digests were conflated | three distinct final digests are reported below, with the coordinate-independent semantic identity delta stated honestly. |

### 11.2 Canonical adapter and consumer state

- Canonical adapter rows: `31/31 LIVE` across all seven families. The six legacy concrete
  Physical Prepared types remain transitional Physical-only; their family rows point to
  `StandardAdapterPreparedStatement` plus the family-specific factory constructor, executor core,
  and behavior/NOT_RUN evidence.
- Shared common provenance chains: `6/6 LIVE`; sharing one concrete standard type does not collapse
  the per-family executor provenance.
- Legacy `SqlTextStatement`/`SqlTextPreparedStatement` production references: `21 CLOSED`; the old
  source references are absent. Their wrapper-private replacement is
  `PreparedStatementResourceState`, not a canonical facade or alias.
- Active production consumer rows: `3536`; checked denominator rows: `3557` (`3536` active + `21`
  historical CLOSED). The Task 2 BASE count `3559` is preserved as historical input, not asserted
  as the Task 4 active-source count.
- Coordinate/binding-independent semantic identities versus `8252a20`: `added=10`, `removed=33`
  (net `-23`). This includes the 21 real legacy-reference removals and the private-core relocation;
  no virtual occurrence or scanner exclusion was added to manufacture the old count.

Final hash vocabulary and values:

```text
CONSUMER_ACTIVE_SOURCE_SHA256=c8bba1b39772438d5e94cfdcc03d44169df4c245637de66527885818d4aff183
CONSUMER_SOURCE_PROJECTION_SHA256=90cacc0be2a227abec57c664419ea3c2b7bb93377c9834ebf77a2b14e3fda41c
CONSUMER_LIVE_TSV_SHA256=a9651d5310a095db8ff8fdc0c367389ea108397bfea90ff0b7c73923b1a80652
```

### 11.3 Seven-family behavior matrix

| Family | Canonical standard behavior | Fresh result |
| --- | --- | --- |
| SQLx SQLite | owned actor query/update/parameter metadata/tables/lifecycle | configured real in-memory SQLite PASS |
| Toasty SQLite | factory -> connection -> statement/prepared -> update/query | configured real in-memory SQLite PASS |
| RBDC | factory -> connection -> prepared setter/query through owned deterministic driver | deterministic fake PASS; external DB NOT_RUN |
| DuckDB native | factory -> connection -> statement/prepared -> update/query | configured in-memory native PASS |
| HTTP SQL | factory/connection/owned family executor rejects absent safe remote configuration | typed `InvalidArgument`, NOT_RUN |
| libSQL native | factory/connection/owned family executor rejects absent valid remote URL/token | typed `InvalidArgument`, NOT_RUN |
| JDBC Agent | factory/connection/owned family executor reports absent runtime/jars | typed error, NOT_RUN |

No external HTTP/libSQL/JDBC Agent/RBDC service is reported PASS. No Cargo topology, druid-admin,
Task 5 holder/proxy, or Druid production file was changed.

### 11.4 Fresh final gates

- wrapper verifier self/live: `8/8 PASS`; `31 LIVE`, `21 CLOSED`, `6` common provenance chains,
  SQLx actor `1`.
- denominator self/live: `39/39 PASS`; Physical `73`, adapters `55`, checked/projected consumers
  `3557`.
- semantic self/adapter-live: `16/16 PASS`; `52/17/0`, normalized and LIVE semantic hashes remain
  `5d99b573...cdbd9` and `45882120...bf21`.
- Task 3 purity self/transitional: `103/103 PASS`; `STANDARD_FILES=29`.
- `cargo test -p druid-wrapper --tests` and
  `cargo test -p druid-wrapper --all-features --tests`: every test binary PASS, including standard
  configured/NOT_RUN paths and SQLx ownership/metadata race cases.
- `cargo test -p druid-rdbc --all-targets --all-features`: `20/20 PASS`;
  `cargo test -p druid-rdbc --doc`: `1/1 PASS`;
  druid-rdbc Clippy with `-D warnings`: PASS.
- `cargo check -p druid-wrapper --all-features --all-targets` and
  `cargo check --workspace --all-targets --all-features`: PASS.
- scoped wrapper Clippy `--no-deps -- -D warnings`, wrapper docs, wrapper doctests (`0`), rustfmt,
  `git diff --check`: PASS.
- The dependency-aware broad wrapper Clippy command is still **not GREEN** solely because of the two
  unchanged Druid Task 5 warnings recorded in Section 8; this round did not modify or conceal them.

### 11.5 Round state and risks

All `1 Critical + 4 Important + 1 Minor` review findings have an implementation disposition of
**ADDRESSED, pending independent re-review**. Task 5 remains blocked. External remote-driver behavior
is honestly NOT_RUN, SQLx result rows remain eager as the accepted Task 7 boundary, and transitional
Physical implementations remain only for the planned Task 5/6 migration window.

## 12. Fix round 1 independent-review correction

Independent review of `8252a20..b4b69ea` returned **FIX_REQUIRED**. The prior Section 11 completion
wording is superseded: C1 and I1 are `PARTIAL`; I2, I3, I4, and M1 are `ADDRESSED`; new Important N1
is open. C1 still lacks deterministic post-factory HTTP SQL/libSQL/JDBC Agent execution and
source-bound behavior evidence, and common ResultSet metadata loses family descriptors. I1 still
allows connection-first Prepared setters and non-SQLx live metadata after owner close. N1 permits a
fabricated set of 21 CLOSED rows and `--emit-live` drops the honest historical set. Task 5 remains
blocked until strict RED/GREEN fixes and independent review clear all three load-bearing gaps.

## 13. Fix round 2/5 implementation (base `b4b69ea`)

This append-only section records the current implementation evidence. It does not override the
review verdict: Task 5 remains blocked until an independent round-2 review returns clear.

### 13.1 Finding disposition and RED/GREEN

| Finding | Fresh RED | Round-2 GREEN |
| --- | --- | --- |
| C1 remote owned behavior and verifier binding | HTTP SQL/libSQL/JDBC Agent stopped before connection construction; comment-only behavior evidence was accepted | Each remote family factory now injects only its external transport/runtime boundary. Deterministic fakes enter the production `create_adapter_core`/connector, concrete family adapter, `standard_executor_core`, and common owned Statement/Prepared/ResultSet path. Each executes update, setter, prepared query, `next`, driver label metadata, live database metadata, close, and post-close rejection. Focused remote tests `3/3 PASS`. Verifier resolves one exact enabled test function, strips comments/literals, requires an observable assertion and the complete remote call chain; self-tests `10/10 PASS` reject comment-only, name-only, and empty assertion evidence. |
| I1 connection-first lifecycle | SQLx/Toasty/RBDC setters returned `Ok` after connection close; DuckDB metadata could not coexist with an already-owned executor | Common and SQLx Prepared mutation/execution now check the shared executor/actor before state changes. All six common-family live metadata objects retain the executor and check it for both standard metadata operations; SQLx metadata checks actor closure. ResultSet/parameter metadata snapshots remain immutable and usable offline. SQLx/Toasty/RBDC/DuckDB focused suites are `6/8/13/11 PASS`; fake remotes cover connection-first, statement-first, double close and live metadata; common executor close-error and deterministic concurrent close/execute tests are `2/2 PASS`. |
| N1 exact CLOSED history | fabricated replacement rows were accepted and `--emit-live` emitted zero CLOSED rows | The verifier loads `8252a20f...` with offline `git show`, selects the exact 21 Task4 SqlText identities, normalizes only state, and authenticates bytes with SHA-256 `bcc50e12e484a78bff19e578e362f25e4a382259be1ec53aadef53f7f01eca14`. Current CLOSED rows must equal that set field-for-field and remain absent from active source. `--emit-live` merges the pinned history with the active scan. Full denominator self-tests are `41/41 PASS`. |

I2, I3, I4, and M1 remain addressed; their existing behavioral and hash vocabulary gates did not
regress.

### 13.2 Canonical matrix and metadata envelope

| Family | Deterministic/configured standard evidence | External status |
| --- | --- | --- |
| SQLx SQLite | real in-memory actor query/update/placeholder/tables/lifecycle and label snapshot | PASS |
| Toasty SQLite | real in-memory query/update/lifecycle; explicit owned result descriptor envelope | PASS |
| RBDC | deterministic driver query/update/labels/lifecycle | PASS; real external DB NOT_RUN |
| DuckDB native | real in-memory query/update/native labels/lifecycle | PASS |
| HTTP SQL | deterministic production transport injection through real family core | PASS fake; external service NOT_RUN |
| libSQL native | deterministic production transport injection through real family core | PASS fake; external service NOT_RUN |
| JDBC Agent | deterministic production runtime connector through real protocol/family core | PASS fake; external jars/service NOT_RUN |

Common query execution now returns an owned `StandardQueryResult` containing rows plus driver labels,
which `StandardAdapterResultSet` turns into an immutable `Arc<ResultSetMetaDataSnapshot>`. RBDC,
DuckDB, HTTP SQL, libSQL, and JDBC Agent preserve concrete labels; Toasty retains a deterministic
unknown descriptor where its current raw driver response exposes values but no labels.

### 13.3 Inventories and final hashes

```text
WRAPPER_STANDARD_ADAPTERS_LIVE=31
WRAPPER_STANDARD_CONSUMERS_CLOSED=21
FAMILY_COMMON_PREPARED_CHAINS_LIVE=6
SQLX_OWNED_ACTOR_LIVE=1
PHYSICAL_ROWS=73
ADAPTER_ROWS=55
ACTIVE_SOURCE_ROWS=3536
CHECKED_ROWS=3557
CONSUMER_ACTIVE_SOURCE_SHA256=80d529382e7ff4f7846584f14329d7f8ebd6e0b50f662d409518d7cbcfe5442b
CONSUMER_SOURCE_PROJECTION_SHA256=18bb9682efb4e8521556f13abbc7b1320f2f2bf2d9b9ccb18a0576b3d02d5221
CONSUMER_LIVE_TSV_SHA256=3a190f9e65c261ce95134c1c7dbcd2558c89a23e3f937df8bfeddc1f60384937
CLOSED_HISTORY_SHA256=bcc50e12e484a78bff19e578e362f25e4a382259be1ec53aadef53f7f01eca14
ROUND2_SEMANTIC_IDENTITY_ADDED=0
ROUND2_SEMANTIC_IDENTITY_REMOVED=0
```

The source projection and checked TSV hashes remain distinct terms. Active count remains `3536`;
the checked count is active plus the exact 21-row historical audit set.

### 13.4 Fresh gates and remaining risk

- wrapper verifier self/live: `10/10 PASS`; `31/21/6/1`.
- denominator self/live: `41/41 PASS`; `73/55/3557`.
- semantic self/adapter-live: `16/16 PASS`; `52/17/0`; hashes remain `5d99...` / `4588...`.
- Task3 purity self/transitional: `103/103 PASS`; `STANDARD_FILES=29`.
- wrapper default and all-feature test matrices: every test binary PASS; all-feature unit tests include
  all three deterministic remote factory paths.
- druid-rdbc all-target/all-feature tests `20/20`, doc `1/1`, and Clippy `-D warnings`: PASS.
- wrapper/workspace all-target/all-feature checks: PASS. Scoped wrapper no-deps Clippy, docs,
  doctests (`0`), rustfmt, and diff check: PASS.
- Dependency-aware broad wrapper Clippy is still **not GREEN** only at the two unchanged Druid Task5
  baseline warnings recorded earlier; no Task5/Druid production file was modified.

All C1/I1/N1 round-1 review gaps now have an implementation disposition of **ADDRESSED, pending
independent re-review**. Real external HTTP SQL/libSQL/JDBC Agent/RBDC services remain honestly
NOT_RUN. No Cargo topology, druid-admin, Task5 holder/proxy, Physical deletion, unsafe, or manual
Send/Sync change was made.

## 14. Fix round 2 independent-review correction

Independent review of `b4b69ea..820438a` returned **FIX_REQUIRED** with one new narrow Important
finding N2. I1, N1, I2, I3, I4, and M1 are `ADDRESSED`; C1 production behavior is addressed, but its
fitness-function subcondition remains `PARTIAL`. The current verifier accepts an otherwise valid
behavior test under `#[cfg(any())]` because it checks nearby test-attribute text without evaluating
the enclosing cfg/cfg_attr/profile. It can likewise count required tokens in a statically dead branch.
The prior Section 13 completion wording is superseded. Task 5 remains blocked until N2 receives
strict RED/GREEN and independent review clear.

## 15. Fix round 3/5 implementation (base `820438a`)

This section records the N2-only implementation and supersedes Section 14 only for current verifier
status. Task 5 remains blocked pending independent round-3 review.

### 15.1 Fresh RED -> GREEN

Fresh fixtures against `820438a` produced two failing groups: behavior tests under `cfg(any())`, a
disabled feature/target, nested disabling `cfg_attr`, and disabled enclosing module were accepted;
required call/assert tokens under `#[cfg(any())]`, `if false`, or `while false` were retained as live
evidence. The failures were policy failures, not parser errors.

The minimal GREEN reuses the denominator verifier's exact `CfgExpr`, `parse_cfg_meta`,
`parse_cfg_expression`, atom collection, and evaluator. The shared evaluator now accepts an explicit
`test_value`; denominator production scans retain `test=false`, while behavior evidence uses
`test=true`. No second simplified cfg grammar was introduced.

Behavior profiles are explicit:

- default families use the recursively expanded `druid-wrapper` default Cargo feature set;
- DuckDB, HTTP SQL, libSQL, and JDBC Agent rows use the all-features set;
- target flags/values come from the current `rustc --print cfg`;
- undeclared/disabled Cargo features evaluate false, known target alternatives evaluate false, and
  an unproven custom cfg atom fails closed;
- actual repository evidence must also occur exactly once in the corresponding
  `cargo test -p druid-wrapper --tests [--all-features] -- --list` inventory.

The token-range analysis evaluates file inner cfg, function attributes, nested cfg_attr-provided test
attributes, enclosing modules/impls, and cfg-disabled statements/blocks. It removes obvious static
dead `if false`, `while false`, and `if true` else bodies before checking assertions and the remote
factory-to-result call chain. Comments and string contents remain absent through the canonical Rust
tokenizer.

Permanent verifier controls now cover:

- reject direct `cfg(any())`, disabled feature, disabled target, nested cfg_attr-disabled body/test,
  disabled enclosing module and impl, and unknown custom cfg;
- reject required calls/assertions present only in cfg-disabled, `if false`, or `while false` ranges;
- accept direct default test, enabled all-feature/target predicates, tokio::test via cfg_attr, and
  required tokens in the live `if true` branch;
- retain all ten prior adversarial controls.

Final verifier self-tests are `14/14 PASS`; the live gate remains `31/21/6/1` and every checked
behavior function is unique in its actual Cargo profile.

### 15.2 Non-regression and final gates

- denominator shared cfg model: `41/41 PASS`; live `73/55/3557`, projection SHA
  `18bb9682efb4e8521556f13abbc7b1320f2f2bf2d9b9ccb18a0576b3d02d5221`.
- checked consumer TSV SHA remains
  `3a190f9e65c261ce95134c1c7dbcd2558c89a23e3f937df8bfeddc1f60384937`;
  adapter/consumer TSV bytes and all production adapter sources are unchanged in this round.
- semantic self/live: `16/16 PASS`, `52/17/0`, hashes `5d99.../4588...`.
- Task3 purity: `103/103 PASS + live29`.
- wrapper default and all-feature complete test matrices: every binary PASS.
- druid-rdbc all-target/all-feature `20/20`, doc `1/1`, Clippy `-D warnings`: PASS.
- wrapper and workspace all-target/all-feature checks, scoped wrapper no-deps Clippy, wrapper docs,
  doctests (`0`), rustfmt, and diff check: PASS.
- dependency-aware broad Clippy remains non-GREEN only at the two unchanged Druid Task5 baseline
  lines and is not reported GREEN.

N2 and the remaining C1 verifier subcondition now have an implementation disposition of
**ADDRESSED, pending independent re-review**. I1/N1/I2/I3/I4/M1 remain addressed. No production
adapter, TSV, Cargo, Druid/admin, Task5, Physical deletion, unsafe, or manual Send/Sync file changed.

## 16. Fix round 3 independent-review correction

Independent review of `820438a..b1f8948` returned **FIX_REQUIRED**. The cfg/profile/Cargo inventory
portion of N2 is addressed, but nested static-dead traversal remains `PARTIAL`: an outer live
`if true` causes the scanner to skip an inner `if false` or `while false`, so a complete dead remote
factory-to-result chain can still satisfy LIVE evidence. All other historical findings remain
addressed. Task 5 remains blocked.

## 17. Fix round 4/5 implementation (base `b1f8948`)

This append-only section records the narrow recursive reachability fix. It does not claim review
clear; Task 5 remains blocked pending independent round-4 re-review.

### 17.1 Permanent full-verifier RED -> GREEN

Four permanent full-verifier controls were added over an HTTP SQL adapter row, rather than testing
only the token helper. Against `b1f8948`, three tests failed with the expected
`VerificationError not raised` result:

- a complete remote chain inside `if true { if false { ... } }`;
- the same chain inside `if true { while false { ... } }`;
- dead-then/live-else-if traversal through nested plain blocks and `loop`, ending in another
  `if false`.

The fourth control was already GREEN and remains so: `if false` retains its live `else`, and an
unknown runtime condition retains tokens from both branches. The final self-test result is
`18/18 PASS`.

### 17.2 Recursive live-token reachability

`static_dead_branch_indexes` was replaced by `static_live_branch_indexes`:

- the starting live-token mask is the behavior-function range minus the existing cfg-disabled
  indexes;
- every retained balanced `{...}` block is scanned recursively, including ordinary blocks and loop
  bodies;
- only an immediately proven literal `if/while true|false` changes reachability;
- `if false` removes the then block and recursively evaluates a retained else/else-if chain;
- `if true` recursively evaluates the then block and removes the complete else/else-if range;
- `while false` removes its body while `while true` recursively scans it;
- unknown runtime predicates preserve both branches and are only traversed for nested literal
  controls.

The implementation does not add a general constant evaluator and does not claim `match` or const
boolean evaluation. Balanced block boundaries and recursive else-if bounds prevent an outer live
branch from skipping its nested controls. Existing Rust tokenization still removes comment/literal
contents, and the shared cfg/profile exclusions are unchanged.

### 17.3 Fresh non-regression evidence

- wrapper verifier self/live: `18/18 PASS`; `31/21/6/1` unchanged;
- denominator self/live: `41/41 PASS`; `73/55/3557`; projection SHA
  `18bb9682efb4e8521556f13abbc7b1320f2f2bf2d9b9ccb18a0576b3d02d5221`;
- semantic self/adapter-live: `16/16 PASS`; `52/17/0`; hashes remain `5d99...` / `4588...`;
- Task 3 purity self/transitional: `103/103 PASS + live29`;
- wrapper default and all-feature complete test matrices: every binary PASS; all-feature library
  tests remain `16/16`, including the three deterministic remote chains;
- druid-rdbc all-target/all-feature `20/20`, doc `1/1`, and Clippy `-D warnings`: PASS;
- wrapper all-feature/all-target check, workspace all-target/all-feature check, scoped wrapper
  no-deps Clippy, wrapper docs, doctests (`0`), rustfmt, Python byte-compile, and diff check: PASS;
- dependency-aware broad wrapper Clippy remains non-GREEN only at the two unchanged Druid Task 5
  baseline lines `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`.

The checked consumer TSV SHA remains
`3a190f9e65c261ce95134c1c7dbcd2558c89a23e3f937df8bfeddc1f60384937`; adapter, consumer, and
semantic TSV bytes are unchanged. Production adapters, Cargo/lock, Druid/admin, Task 5, Physical
names, unsafe, and manual Send/Sync are unchanged. N2 now has an implementation disposition of
**ADDRESSED, pending independent re-review**.
