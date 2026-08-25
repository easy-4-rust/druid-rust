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

## 18. Fix round 5/5 implementation (base `2a4a249`)

This append-only section records the N2 path-coherence implementation. It does not claim review
clear; Task 5 remains blocked pending the independent final re-review/breaker ruling.

### 18.1 Full-verifier RED -> GREEN

The existing verifier self-test contract remains exactly `18` test methods. Fresh full-verifier
subcases first produced 16 expected `VerificationError not raised` failures against the round-4
implementation. They cover:

- mutually exclusive and nested runtime `if/else` branches, early return, match-arm splitting, and
  a complete chain separated from its assertion;
- runtime/maybe-empty loops, uncalled closure/helper/async block/async closure, cfg-dead code, and
  comment/string tokens;
- uninvoked `macro_rules!`, multiple macro arms, uninvoked macro-2, and a macro invocation whose
  call-site contains only the macro name;
- wrong call order, repeated `database_meta_data` attempting to satisfy exact `meta_data`, and a
  seven-branch path explosion.

Two same-class gaps found during implementation received their own incremental RED -> GREEN cycle:
macro-2 parameter delimiters were initially excluded without excluding the following expansion
body, and an uncalled `async ||` closure was initially not recognized as deferred execution. The
final self-test result is `18/18 PASS`, including all of these permanent subcases and the prior
nested literal/cfg controls.

### 18.2 Bounded feasible-path model

The verifier now builds ordered `BehaviorPathSummary` alternatives instead of flattening one live
token union:

- literal `true/false` selects one branch; runtime `if/else` and parsed `match` arms fork; nested
  controls recurse; `return` terminates only its candidate path; suffix tokens join only
  fallthrough paths;
- runtime `while` and `for` bodies cannot supply evidence because zero iterations are feasible;
  `loop` and `while true` bodies reach a suffix only through a proven `break`; divergent candidates
  cannot satisfy evidence;
- candidate summaries are deduplicated and capped at 64; exceeding the cap fails closed;
- complete `macro_rules!` and macro-2 definition ranges, nested helper function bodies, uncalled
  closures, and unawaited async blocks contribute no execution tokens;
- non-assertion function-like macro arguments are considered only after exact Cargo test inventory
  proves the named test is compiled; expansion bodies are never simulated. Direct call-site
  arguments remain analyzable, while an invocation name alone supplies no behavior;
- approved direct assertion macros (`assert*`, `debug_assert*`, `assert_impl_all!`, `matches!`) are
  recognized by exact invocation tokens, not comments, literals, names, or substrings.

Every remote candidate path is checked independently for exact call tokens in the observed test
order: factory create -> Statement construction/update -> Prepared construction/setter/query ->
ResultSet next/metadata -> live DatabaseMetaData -> Connection close. The complete ordered chain
and an approved assertion must occur on the same feasible path. Exact token matching prevents
`database_meta_data` from impersonating `meta_data`; the scan cursor prevents reversed order from
passing.

This is intentionally not a general Rust interpreter. Calls in unknown/maybe-zero loop bodies,
uncalled or indirectly scheduled closures/futures, helper bodies, and macro expansions are ignored;
evidence tests that need those abstractions must expose the required chain directly. Only an async
block followed immediately by `.await` is traversed. These boundaries are fail-closed for Task 4
evidence.

### 18.3 Real evidence and non-regression

The HTTP SQL, libSQL, and JDBC Agent behavior tests needed no source change. Each compile-backed
test resolves to exactly one `fallthrough` path with `10/10` ordered chain calls and an observable
assertion. Fresh gates:

- wrapper verifier self/live: `18/18 PASS`; `31/21/6/1` unchanged;
- denominator self/live: `41/41 PASS`; `73/55/3557`; projection SHA
  `18bb9682efb4e8521556f13abbc7b1320f2f2bf2d9b9ccb18a0576b3d02d5221`;
- semantic self/adapter-live: `16/16 PASS`; `52/17/0`; hashes remain `5d99...` / `4588...`;
- purity self/transitional: `103/103 PASS + live29`;
- wrapper default/all-feature complete test matrices, druid-rdbc all-target/all-feature `20/20`,
  druid-rdbc doc `1/1`, druid-rdbc Clippy, wrapper/workspace all-feature checks, wrapper scoped
  no-deps Clippy, wrapper docs/doctests, rustfmt, Python byte-compile, and diff check: PASS;
- dependency-aware wrapper Clippy reproduces only the two unchanged Task 5 baseline warnings at
  `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`.

Adapter/consumer/semantic TSV SHA-256 values remain respectively `fda28b7e...`, `3a190f9e...`, and
`45882120...`; their bytes are unchanged. Production adapters, Cargo/lock, Druid/admin, Task 5,
Physical names, unsafe, and manual Send/Sync are unchanged. N2 has an implementation disposition of
**ADDRESSED, pending independent final re-review**.

## 19. Breaker recovery implementation (base `141a203`)

This append-only section records the user-approved straight-line recovery after the round-5 breaker.
It supersedes Section 18 only for the verifier architecture and current submission state. It does not
claim review clear; Task 5 remains blocked pending an independent breaker re-review.

### 19.1 Fresh full-verifier RED -> GREEN

On the unchanged `d261174` verifier at recovery base `141a203`, the historical self-test suite was
`18/18 PASS`, but the three independent breaker fixtures were all accepted:

```text
runtime-empty-else=ACCEPT
match-empty-arm=ACCEPT
runtime-early-pass=ACCEPT
```

The permanent full-verifier controls first produced the expected
`VerificationError not raised` failures for those three cases plus complete/empty runtime branches,
complete branches on both sides, literal/static branches, direct early exits, and short-circuit
`&&`/`||`. Incremental RED -> GREEN controls also closed `?`, loop/break/continue, closure positions,
awaited/unawaited async blocks, local helper/type/macro definitions, user/qualified/external
function-like macros, assertion-macro definition/import/glob shadows, chain/assertion separation,
`macro_use`, wrong order, marker substrings, ignored/should-panic tests, and the prior cfg/dead-token
cases.

Fresh recovery results are:

```text
runtime-empty-else=REJECT: straight-line profile forbids branch `if`
match-empty-arm=REJECT: straight-line profile forbids branch `match`
runtime-early-pass=REJECT: straight-line profile forbids branch `if`
verifier self-test methods=18/18 PASS
```

The existing 18 test methods remain and their adversarial subcase matrix is expanded. The only
full-verifier positive fixture is the minimal direct straight-line 10-call chain plus assertion.

### 19.2 One strict evidence policy

The bounded `BehaviorPathSummary` interpreter, path explosion budget, branch/match/loop composition,
and existential `complete_paths` acceptance are removed. There is no alternative path-acceptance
policy left in the verifier.

For every LIVE adapter evidence row, the verifier now applies one order:

1. Resolve the exact evidence file/function, evaluate file/enclosing/function cfg under the declared
   default or all-feature profile, and require one exact occurrence in Cargo's actual `-- --list`
   inventory.
2. Reject ignored/should-panic tests and any body-level cfg/static branch, runtime branch/match,
   loop, short-circuit operator, return/break/continue, `?`, closure/async block, local item/helper,
   unsafe/try/yield construct, or unapproved function-like macro. Diagnostics name the concrete
   forbidden construct.
3. Permit only the finite built-in assertion allowlist at direct invocation sites. Qualified,
   external, locally defined, explicitly imported, or ambiguous-glob assertion macros fail closed;
   macro definitions/expansions never supply evidence.
4. Inspect the one direct fallthrough body token sequence. Remote rows must contain the exact ordered
   ten-call `ConnectionFactory::create` through `Connection::close` chain and a direct approved
   assertion in that same body.

This intentionally proves a narrow evidence profile rather than interpreting Rust control flow.

### 19.3 Minimal real evidence-test adaptation

The three load-bearing remote evidence tests already met the strict profile and their source was not
changed. Fresh all-feature execution is `3/3 PASS` for HTTP SQL, libSQL, and JDBC Agent.

Three other LIVE evidence tests contained non-evidence convenience syntax and were mechanically
straightened without production changes:

- DuckDB and Toasty replace a two-item `for` setter loop with the same two ordered setter calls;
- RBDC replaces `vec!` in the expected value with the equivalent one-element array literal;
- the two DataSource Wrapper rows replace external `assert_impl_all!` use in their evidence function
  with direct canonical trait-method function-pointer declarations and built-in `assert_ne!` checks.

Focused DuckDB, Toasty, RBDC, and Wrapper identity tests are each `1/1 PASS`. All other LIVE evidence
functions already satisfy the same profile.

### 19.4 Fresh recovery gates

- wrapper verifier self/live: `18/18 PASS`; `31 LIVE / 21 CLOSED / 6 family chains / 1 SQLx actor`;
- reviewer breaker probes: all three reject with construct-specific diagnostics;
- denominator self/live: `41/41 PASS`; `73 / 55 / 3557`, projection SHA
  `18bb9682efb4e8521556f13abbc7b1320f2f2bf2d9b9ccb18a0576b3d02d5221`;
- semantic self/adapter-live: `16/16 PASS`; `52 / 17 / 0`, hashes remain
  `5d99b573...cdbd9` / `45882120...bf21`;
- Task 3 purity self/transitional: `103/103 PASS + live29`;
- wrapper default and all-feature complete test matrices PASS; all-feature library remote evidence is
  `3/3 PASS`;
- druid-rdbc all-target/all-feature tests, doc test, and Clippy `-D warnings` PASS;
- wrapper/workspace all-target/all-feature checks, wrapper no-deps Clippy, wrapper docs/doctests,
  Python byte-compile, rustfmt, and diff checks PASS.

The dependency-aware wrapper Clippy command remains non-GREEN only at the same two untouched Task 5
baseline lines `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`; it is not
reported GREEN.

### 19.5 Scope and submission state

Production adapters, verification TSVs, Cargo/lock, Druid/admin, Task 5, Physical names, unsafe, and
manual Send/Sync are unchanged. Exact checked TSV SHA-256 values remain:

```text
ADAPTER_TSV_SHA256=fda28b7ea4de21a7611cd353609c792654c7ac3e2ac6ab27ae4ecc19adfa186d
CONSUMER_TSV_SHA256=3a190f9e65c261ce95134c1c7dbcd2558c89a23e3f937df8bfeddc1f60384937
SEMANTIC_TSV_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

N2 and the remaining C1 verifier subcondition have a recovery implementation disposition of
**ADDRESSED, pending independent breaker re-review**. Task 5 remains blocked until that review returns
`REVIEW CLEAR`.

## 20. Runtime Trace Recovery implementation（base `876d438`）

This append-only section supersedes Section 19 only for the load-bearing behavior-evidence
architecture. It does not rewrite the historical straight-line rejection and does not claim review
clear. Task 5 remains blocked pending a fresh independent runtime-trace re-review.

### 20.1 Fresh RED fixed before implementation

The unchanged `23a770f` straight-line verifier started at `18/18 PASS`, while six permanent recovery
checks failed for the intended missing contract:

- dropped async Futures、`std::process::exit(0)`、bare `matches!` and an assertion before the chain
  were all incorrectly accepted as behavior evidence;
- no typed trace-output validator existed;
- no source-hygiene gate prevented a fake marker or fake transport from recording stages.

The recovery self-test was then reduced from the obsolete 18-method Rust control-flow interpreter
suite to a focused 14-method runtime protocol suite. The final result is `14/14 PASS`; the removed
path/token acceptance is not retained as a second behavior oracle.

### 20.2 Unique typed protocol and opaque API

The only accepted protocol is `task4-runtime-trace-v1`, with this exact 11-stage sequence:

```text
FactoryCreate -> ConnectionCreate -> StatementCreate -> StatementUpdate -> PreparedCreate ->
ParameterSet -> PreparedQuery -> ResultNext -> ResultSetMetadata -> DatabaseMetadata ->
ConnectionClose
```

- private `FamilyId`、`TraceStage`、`TraceEvent` and `RuntimeTraceEmitter` construct events;
- every event carries the same family and monotonic connection/executor instance id;
- the seven factories carry `Option<RuntimeTraceEmitter>` with normal production construction set to
  `None`（no allocation or event lock on the normal path）；
- evidence setup returns only opaque `TraceProbe` and `TraceExpectation`; neither exposes a
  constructor、field、event accessor、family accessor or record API;
- `verify_and_emit(probe, expectation)` consumes both values and is the only marker serializer；it
  emits one `DRUID_TASK4_RUNTIME_TRACE` record only after typed sequence validation succeeds;
- common `OwnedAdapterExecutor` / Statement / PreparedStatement / ResultSet / live DatabaseMetaData
  record at their real success/attempt execution points；SQLx uses the same typed core through its
  actor、owned Prepared/ResultSet/live metadata path;
- common and SQLx close record only after the real close core succeeds and exactly once；the SQLx
  actor rechecks closed state after acquiring its serialization lock.

External HTTP/libSQL/JDBC and deterministic RBDC doubles still replace only external I/O. They do
not receive an emitter/probe and contain no marker/stage/record call. Result labels、returned values、
update counts、live metadata outcome、post-close rejection and immutable snapshot survival remain
asserted by the Rust evidence tests（including RBDC's truthful typed unsupported product-name
outcome）。

### 20.3 Exact runtime verifier and manifest

`rdbc-adapter-runtime-traces.tsv` contains exactly 31 source/target rows:

- 29 adapter rows map to seven unique `RUNTIME_TRACE` exact Cargo test/profile/family/version/evidence
  specs；
- the two DataSource Wrapper rows map to one independent `COMPILE_IDENTITY` exact test and carry no
  fake runtime family/version/id.

For every unique spec the verifier runs `cargo test -p druid-wrapper <profile> <target> <test-id>
-- --exact --nocapture`, requires exit 0 and a completed libtest `1 passed / 0 failed / 0 ignored`
summary, and then requires exactly one library-owned marker. JSON keys、version、evidence id、family、
instance and all events are exact. Missing、duplicate、out-of-order、wrong-family、wrong-instance、
wrong-version、wrong-id、no-marker and duplicate-marker fixtures fail closed. A passing early process
exit or dropped Future has no marker and therefore fails.

Static analysis now has only two roles：canonical source/core identity and anti-forgery. It rejects
marker/event/record construction in the exact evidence test or its `cfg(test)` fake transport region,
allows record callsites only in the reviewed production factory/common/SQLx actor modules, and allows
marker serialization only in `task4_runtime_trace.rs`. It no longer interprets Rust control flow or
accepts source tokens as behavior proof.

### 20.4 Seven-family exact evidence and configured status

| Family | Exact profile/target | Fresh runtime result | External status |
| --- | --- | --- | --- |
| SQLx SQLite | default integration | real in-memory SQLite trace `1/1 PASS` | configured PASS |
| Toasty SQLite | default integration | real in-memory SQLite trace `1/1 PASS` | configured PASS |
| RBDC | default integration | deterministic driver trace `1/1 PASS` | real external DB NOT_RUN |
| DuckDB | all-features integration | real native memory trace `1/1 PASS` | configured PASS |
| HTTP SQL | all-features lib unit | production factory + deterministic transport trace `1/1 PASS` | real service NOT_RUN |
| libSQL | all-features lib unit | production factory + deterministic transport trace `1/1 PASS` | real service NOT_RUN |
| JDBC Agent | all-features lib unit | production factory + deterministic connector trace `1/1 PASS` | jars/service NOT_RUN |

No external service was upgraded from NOT_RUN. The deterministic production-bound trace is evidence
for adapter behavior, not evidence that the unavailable external service was contacted.

### 20.5 Fresh gates、denominators and hashes

- wrapper verifier self/live：`14/14 PASS`；`31 / 21 / 6 / 1` plus runtime families `7` and
  nonbehavior identity rows `2`；
- denominator self/live：`41/41 PASS`；Physical `73`、adapter `55`、active consumers `3536`、checked
  consumers `3557`（active + exact pinned CLOSED21）；
- semantic self/adapter-live：`16/16 PASS`；`52 / 17 / UNKNOWN 0`；hashes remain
  `5d99b573...cdbd9` / `45882120...bf21`；
- purity self/transitional：`103/103 PASS + live29`；
- all seven exact trace tests and the exact DataSource identity test PASS through the live verifier；
- wrapper default and all-feature complete test matrices：every test binary PASS；
- wrapper default/all-feature all-target checks、workspace all-target/all-feature check、scoped
  wrapper no-deps Clippy `-D warnings`、wrapper docs/doctests、Python byte-compile、rustfmt and diff
  check：PASS；
- druid-rdbc all-target/all-feature tests `20/20`、doc `1/1` and Clippy `-D warnings`：PASS；
- dependency-aware wrapper Clippy remains deliberately **not GREEN** only at the same untouched Task
  5 lines `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`.

Hash vocabulary after the coordinate-bearing source refresh:

```text
ADAPTER_SOURCE_PROJECTION_SHA256=4090959c7bd6e0858f870b651ea3e36e00a602f91804783430c61baa59c184c8
ADAPTER_LIVE_TSV_SHA256=350f6b6350f7c75fbf88aa15872f14faaa7cdb9c90faa0c3d5d03ea8442b6410
RUNTIME_TRACE_TSV_SHA256=a2b5f213c28b2363679d30967014dd6bd6facae8f12850d1e101def522f61f4d
CONSUMER_ACTIVE_SOURCE_SHA256=fbc3fe58fa5d6a6c4cb317bc4e0d149f16343b44135335bac8fd1f0bc2f67716
CONSUMER_SOURCE_PROJECTION_SHA256=d6f81a2754bd8010cc4267f5b0cede5fbf94b6b8e0e11debd0a4ebf4a3f2fb58
CONSUMER_LIVE_TSV_SHA256=241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc
SEMANTIC_LIVE_TSV_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

The consumer identity denominator remains `3536 active + 21 CLOSED`; the TSV byte change is the
honest token-coordinate movement caused by Task 4 trace fields/calls. No scanner exclusion or virtual
occurrence was introduced.

### 20.6 Scope and submission state

This recovery changes only Task 4 wrapper factories/adapters/common owned handles/SQLx actor,
their evidence tests, verifier/self-tests, coordinate-bearing evidence TSVs, and append-only SDD
artifacts. It does not change Cargo topology/lock、Task 5 Druid holder/proxies、Physical deletion、
druid-admin、unsafe or manual Send/Sync. C1/N2 have an implementation disposition of **ADDRESSED,
pending independent runtime-trace re-review**. Task 5 remains blocked until that review returns
`REVIEW CLEAR`.

## 21. Typed runtime-trace independent-review correction（append-only）

Independent review of `876d438..f0409af` returned **FIX_REQUIRED** with `1 Critical + 2 Important
+ 0 Minor`。The prior Section 20 implementation disposition is superseded only for its verifier-proof
claim：C1/N2 are **NOT ADDRESSED / load-bearing**，while current production family behavior、I1、I2、
I3、I4、M1 and N1 remain **ADDRESSED with no reviewed regression**。

- Critical：repository-authored helper/module/macro output can forge a structurally valid marker and
  a fake libtest summary before `process::exit(0)`；record provenance is also bypassable from an
  allowed-file helper or an unapproved UFCS call。
- Important：the 31-row manifest is validated only after deduplication，so swapping complete HTTP SQL
  and libSQL family/test/id bindings preserves the same seven-spec set。
- Important：the exact Cargo runner is not offline/locked，has no hard timeout/process-group cleanup，
  and has no bounded stdout/stderr policy。

Review package `review-876d438..f0409af.diff` is byte-exact for the reviewed commit and has SHA-256
`6a30d35384382d894f262f71cab7746d214a3e317311a287b4a0453f8a2b04c9`。The checked typed trace core
itself retains its valid 11-stage family/instance/close invariants，and all seven current evidence
tests remain genuine production-bound tests；the open defect is provenance enforcement under
adversarial mutation。Fix round 1/5 owns only complete source closure + caller provenance/challenge，
row-by-row family binding，and bounded offline runner hardening。Task 5 remains blocked pending a new
independent `REVIEW CLEAR`。

## 22. Runtime-trace fix round 1/5 implementation（base `f0409af`）

This append-only section records the scoped fix for the `1 Critical + 2 Important` review
correction. It does not claim review clear；Task 5 remains blocked pending a fresh independent
re-review。

### 22.1 Architecture choice and permanent RED -> GREEN

The minimum reviewed design uses a complete repository source closure、typed caller provenance and a
per-run challenge，rather than introducing a new Cargo evidence feature/external testkit surface。
An external harness would require moving or publicly feature-gating the private RBDC driver fake and
three remote transport/connector seams；that is materially larger than the provenance-only finding。

Five new self-test groups first failed against `f0409af`：

- a sibling `mod helper` could emit computed marker/fake libtest text and call `process::exit(0)`；
- a helper in an allowed production file and an unapproved UFCS record call were accepted；
- swapping every HTTP SQL/libSQL runtime family binding preserved the same deduplicated spec set；
- Cargo lacked offline/locked flags and no bounded process runner existed；
- marker validation had neither a challenge nor caller-region rules。

The final verifier self-test is `20/20 PASS`。Permanent tests also execute a real hanging parent that
spawns a child and prove process-group termination within the test deadline，exercise the output cap，
and reject a same-file caller line outside the exact production function region。

### 22.2 Complete source closure and output ownership

- integration evidence starts at its exact `tests/<target>.rs`；unit evidence resolves the exact
  `cfg(test)` module source；
- conventional `mod`、inline module content、literal `#[path]` and literal `include!` edges are
  recursively closed；missing、ambiguous、dynamic include and repository-root escape fail closed；
- the corresponding production library closure is recursively rooted at `src/lib.rs`，so unrelated
  CLI binary output is not confused with the linked library；
- the exact evidence root may call only `enable_task4_runtime_trace` / `verify_and_emit` inside its
  manifest-bound test function；reachable helpers/fakes cannot access marker output、record、verify、
  emitter、event、stage、challenge or process termination APIs；
- the sole production marker/output sink remains `task4_runtime_trace.rs`。Production library modules
  cannot print evidence、open process stdout/stderr directly、use raw output descriptors or terminate
  the process。

This closes the reviewer helper-module and fake completion reproductions without accepting a repo-
printed libtest summary as proof。

### 22.3 Typed caller provenance and Rust privacy boundary

`RuntimeTraceEmitter::record` and every stage wrapper hop now use `#[track_caller]`。Each event adds
`caller_file` and `caller_line`；the live validator requires：

1. the exact family/stage production file category；
2. the exact source line containing that stage call；
3. that line lies inside the expected factory/common-handle/SQLx actor function region。

The static gate recognizes both method and UFCS syntax and compares the entire record-callsite
multiset/count against the reviewed production set；a helper、macro or function-pointer relocation
changes the multiset。It also requires every definition in the emitter -> dyn `StandardExecutor` /
SQLx actor chain to carry `#[track_caller]`。

Rust module visibility cannot express a friend set spanning sibling factory、common handle and SQLx
actor modules。Therefore the emitter remains `pub(crate)` internally，while external tests receive no
emitter and crate-internal test/fake use is prevented by the complete source closure、exact static
callsite multiset and runtime caller region together。No event/family constructor or probe accessor
became public。

### 22.4 Per-run challenge and bounded runner

- each runtime execution receives a fresh 256-bit hex challenge；only the typed `verify_and_emit`
  serializer reads it and includes it in the exact marker schema；an old/wrong challenge fails；
- runtime completion now requires Cargo exit 0 plus exactly one challenge-matching typed marker；
  repo-authored `test result: ok` text is neither parsed nor trusted；
- the two DataSource rows remain nonbehavior `COMPILE_IDENTITY` and use offline/locked compile-only
  `--no-run` evidence with zero marker；
- every Cargo command uses `--offline --locked` and its fixed default/all-feature target；
- each process runs in an isolated process group with a 600-second cold-build hard deadline and a
  1 MiB combined stdout/stderr cap；timeout/overflow kills the process group and descendants before
  returning a stable `VerificationError`。The first 120-second probe correctly killed a cold
  DuckDB/Turso build；the documented 600-second cap preserves fail-closed behavior while allowing the
  measured cold all-feature build（about 3m20s）。

### 22.5 Row-by-row binding and live evidence

Before deduplicating to eight executions，all 31 rows derive an immutable family from their adapter
source path and compare this exact tuple：target、test id、profile、category、family、version、evidence
id and recorded behavior evidence。HTTP SQL/libSQL permutation、alias、missing、duplicate、extra and
reordered rows fail before execution。

Fresh runtime evidence remains：

```text
WRAPPER_STANDARD_ADAPTERS_LIVE=31
WRAPPER_STANDARD_CONSUMERS_CLOSED=21
FAMILY_COMMON_PREPARED_CHAINS_LIVE=6
SQLX_OWNED_ACTOR_LIVE=1
RUNTIME_TRACE_FAMILIES_LIVE=7
NONBEHAVIOR_IDENTITY_ROWS=2
```

All seven family tests emit one challenge-bound 11-stage marker with exact caller provenance；the
external HTTP SQL/libSQL/JDBC services and real external RBDC database remain honestly NOT_RUN。

### 22.6 Fresh full gates、hashes and submission state

- verifier self/live：`20/20 PASS` plus `31/21/6/1/trace7/identity2`；
- denominator self/live：`41/41 PASS`，Physical `73`、adapter `55`、active `3536`、checked `3557`；
- semantic self/live：`16/16 PASS + 52/17/0`；purity：`103/103 PASS + live29`；
- wrapper complete default and all-feature matrices：every test binary PASS；
- scoped wrapper no-deps Clippy、workspace all-target/all-feature check、druid-rdbc `20/20` + doc
  `1/1` + Clippy、wrapper docs/doctests、Python byte-compile、rustfmt and diff check：PASS；
- dependency-aware wrapper Clippy remains deliberately non-GREEN only at the same untouched Task 5
  lines `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`。

The evidence TSV bytes do not change in this round：

```text
ADAPTER_LIVE_TSV_SHA256=350f6b6350f7c75fbf88aa15872f14faaa7cdb9c90faa0c3d5d03ea8442b6410
RUNTIME_TRACE_TSV_SHA256=a2b5f213c28b2363679d30967014dd6bd6facae8f12850d1e101def522f61f4d
CONSUMER_LIVE_TSV_SHA256=241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc
SEMANTIC_LIVE_TSV_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

The round changes only the typed trace/caller chain、verifier/self-tests and append-only SDD
artifacts。No Cargo/lock、Task 5、Physical deletion、admin、unsafe or manual Send/Sync change is
included。The three review findings have an implementation disposition of **ADDRESSED, pending
independent re-review**；Task 5 remains blocked。

## 23. Compiler Coverage Evidence Recovery implementation（base `f4c2711`）

This append-only section supersedes Section 22 only for the load-bearing completion/trust claim.
The typed trace protocol and seven production-bound evidence tests remain unchanged；the marker、
challenge text、libtest summary and test/helper output are diagnostic only. Task 5 remains blocked
pending a fresh independent review.

### 23.1 Strict RED -> GREEN evidence

The unchanged `f4c2711` verifier had `20/20 PASS` but no compiler-coverage contract. Ten permanent
coverage groups plus two parent-exit process-group subcases first produced `12` expected failures in
a `31`-test run：

- a structurally matching marker with zero production coverage；
- a test that only creates/drops Futures；
- an exit-zero path that never reaches typed validation success；
- coverage from another exact test/family；
- combined or stale profile/report evidence；
- a zero-count common stage、missing `verify_and_emit` success region、wrong family factory and
  wrong source/function identity；
- duplicate/reordered manifest bindings；
- timeout/output overflow after the direct parent had already exited。

The first GREEN pass was `31/31`. Scoped self-review then found one additional cleanup boundary：a
successful direct parent could exit after spawning a descendant that redirected both pipes. That
new permanent case failed against the first implementation，then passed after final cleanup always
targets the start-time PGID. Final self evidence is `31/31 PASS` both normally and with
`-W error::ResourceWarning`；no pipe/resource warning remains.

### 23.2 Compiler coverage is the trust anchor

Every `RUNTIME_TRACE` EvidenceSpec now runs exactly one command shape：

```text
cargo llvm-cov test --json --output-path <unique>/coverage.json \
  --offline --locked --color never -p druid-wrapper \
  <default|--all-features> <--lib|--test exact-target> <exact-test-id> \
  -- --exact --nocapture
```

- each execution owns a fresh `TemporaryDirectory` and a unique `CARGO_TARGET_DIR`；no profraw、
  profdata、build dir or report path is reused across exact tests；
- the verifier requires fresh non-empty `.profraw` files、exactly one fresh `.profdata` and one fresh
  bounded JSON report beneath that run directory；missing tool/profile/report is
  `NOT_RUN/ERROR`，never PASS；
- command output is capped at `1 MiB`，JSON at `32 MiB`，and each cold build/test has a `600 s`
  deadline；
- the PGID is captured immediately after `Popen(start_new_session=True)` and is killed in normal、
  timeout、overflow and exception cleanup even if the direct cargo parent has already exited；pipes
  are closed deterministically and the direct child is reaped；
- `cargo-llvm-cov 0.8.7` successfully resolved the Rust toolchain LLVM programs and exported LLVM
  coverage JSON `3.1.0` under rustc LLVM `22.1.6`。No global/shared target workaround was used。

`verify_and_emit` still consumes opaque typed probe/expectation values and validates the exact
same-family、same-instance、ordered 11-stage trace. Acceptance now requires compiler coverage of its
final `Ok(())` success region. `DRUID_TASK4_RUNTIME_TRACE` remains useful diagnostics；the verifier
does not set a per-run output challenge and does not parse marker/libtest text for completion.
Complete source closure、callsite multiset and output ownership remain only defense-in-depth.

### 23.3 Checked manifest and exact identities

`rdbc-adapter-runtime-traces.tsv` has a new field-exact coverage binding：

- all `29 RUNTIME_TRACE` rows carry `task4-compiler-coverage-v1` plus the same ordered 14-requirement
  set；
- the two DataSource Wrapper rows remain `COMPILE_IDENTITY` and carry `-` for runtime coverage；
- all 31 rows still bind source/trait -> exact target/test/profile/category/family/trace version /
  evidence id before the eight executions are deduplicated。

The canonical `rdbc-adapter-coverage-regions.json` contains seven ordered family records and exactly
`14` requirements per family (`98` total)：exact test identity、factory create、connection create、
family executor、Statement create/update、Prepared create/set/query、ResultSet next/metadata、live
DatabaseMetaData、connection close and typed validation success. Every requirement records exact
source path、SHA-256、source function、full source line、occurrence、column and positive minimum count.
The manifest rejects missing/extra/duplicate/reordered families or requirements and any wrong
test/profile/version/evidence/source/function/region binding.

Compiler filenames are normalized only by exact path-component prefix removal；substring file or
function identities are never accepted. Rust async lowering does not always assign a standalone
counter to each straight-line call line. In that case the verifier truthfully requires a positive
compiler region/count inside the one exact enclosing source function while the hashed manifest and
typed callsite multiset pin the call line；the separately covered typed-validation success region
proves the ordered stages completed. It does not claim that LLVM emitted an independent counter for
every source line.

The exact-test identity requirement is exclusive：the expected test must have a positive count and
all other six evidence-test identities must be zero or absent. This rejects another test/family and
combined reports in addition to the fresh unique-directory checks.

### 23.4 Seven-family exact evidence

Each family was first run independently during implementation and then all seven were rerun through
the final live verifier after the normal-exit PGID correction：

| Family | Profile / exact target | Compiler evidence | External boundary |
| --- | --- | --- | --- |
| SQLx SQLite | default integration | `14/14 PASS` | configured PASS |
| Toasty SQLite | default integration | `14/14 PASS` | configured PASS |
| RBDC deterministic driver | default integration | `14/14 PASS` | real external DB NOT_RUN |
| DuckDB memory | all-features integration | `14/14 PASS` | configured PASS |
| HTTP SQL deterministic transport | all-features lib | `14/14 PASS` | real service NOT_RUN |
| libSQL deterministic transport | all-features lib | `14/14 PASS` | real service NOT_RUN |
| JDBC Agent deterministic connector | all-features lib | `14/14 PASS` | jars/service NOT_RUN |

The final live verifier output is：

```text
WRAPPER_STANDARD_ADAPTERS_LIVE=31
WRAPPER_STANDARD_CONSUMERS_CLOSED=21
FAMILY_COMMON_PREPARED_CHAINS_LIVE=6
SQLX_OWNED_ACTOR_LIVE=1
RUNTIME_TRACE_FAMILIES_LIVE=7
RUNTIME_TRACE_PROTOCOL=task4-runtime-trace-v1
COMPILER_COVERAGE_FAMILIES_LIVE=7
COMPILER_COVERAGE_PROTOCOL=task4-compiler-coverage-v1
NONBEHAVIOR_IDENTITY_ROWS=2
```

### 23.5 Fresh final gates and hashes

- wrapper verifier self/live：`31/31 PASS` plus the exact output above；
- denominator self/live：`41/41 PASS`，Physical `73`、adapter `55`、checked consumers `3557`；
- semantic self/live：`16/16 PASS + 52/17/0`，and adapter-live reran the complete compiler-coverage
  verifier before emitting its counts；
- purity self/transitional：`103/103 PASS + live29`；
- wrapper default and all-feature complete test matrices：every test binary PASS；default/all-feature
  all-target checks and workspace all-target/all-feature check PASS；
- `druid-rdbc` all-target/all-feature tests `20/20`、doc `1/1` and Clippy `-D warnings` PASS；
- wrapper scoped no-deps Clippy、docs/doctests、Python byte-compile/Black、rustfmt and diff check
  PASS；
- dependency-aware wrapper Clippy remains deliberately non-GREEN only at the two unchanged Task 5
  lines `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`。

```text
ADAPTER_LIVE_TSV_SHA256=350f6b6350f7c75fbf88aa15872f14faaa7cdb9c90faa0c3d5d03ea8442b6410
RUNTIME_TRACE_TSV_SHA256=e55610f3b29b45d1144a903441f64bd20a5b6444a1b388162ce96ad35eb6ceb4
COVERAGE_REGION_MANIFEST_SHA256=3bc58dcd295464f9a6f32db251ace8c46c37be2a796f8e4a31ed9139e725f43b
CONSUMER_LIVE_TSV_SHA256=241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc
SEMANTIC_LIVE_TSV_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

### 23.6 Scope and submission state

The recovery changes only the Task 4 verifier/self-tests、runtime/coverage evidence manifests and
append-only Task 4 report/ledger. `task4_runtime_trace.rs` and all production adapters/tests remain
byte-unchanged from `f4c2711`；no Cargo/lock、Task 5、Physical deletion、admin、unsafe or manual
Send/Sync change is included. C1/N2 have an implementation disposition of **ADDRESSED, pending
independent review**. This is not `REVIEW CLEAR`；Task 5 remains blocked and no push is authorized.

## 24. Compiler-coverage independent-review correction（append-only）

Independent review of `f4c2711..d942cda` corrected the Section 23 completion claim. The prior
same-process marker/challenge path、31-row binding、offline/locked runner、missing/stale/combined
artifact handling and POSIX PGID cleanup are **ADDRESSED**. One new load-bearing Critical remains：

```python
return max(point_counts or function_counts, default=0)
```

When the exact manifest point has count `0` or is absent，the fallback accepts any positive region
inside the enclosing source function. Fresh review fixtures were accepted with
`TRACE_VALIDATION_SUCCESS` or `STATEMENT_UPDATE` at zero while an unrelated sibling region was
positive. Thus compiler coverage became the right trust source but the selected coverage identity
was still fail-open. The final review verdict for `d942cda` is **FIX_REQUIRED**；C1/N2 remains open
and Task 5 remains blocked.

## 25. Compiler Coverage exact-function fix round 2/5（base `d942cda`）

This append records only the exact-function recovery for the Section 24 Critical. It does not claim
independent `REVIEW CLEAR`.

### 25.1 Fresh RED -> GREEN

Three same-source-function fixtures first failed against `d942cda`：

- exact test identity count `0` with an unrelated positive sibling region；
- StatementUpdate target count `0` with its caller/source function otherwise covered；
- typed validation-success target count `0` while another `verify_and_emit` region was positive。

All three were incorrectly accepted. A fourth fresh fixture duplicated the same compiler function
identity and was also accepted. After the implementation，the permanent suite additionally rejects
missing exact functions、wrong source/hash/compiler function、stale reports and missing/duplicate
source anchors. Final verifier self-test result is `38/38 PASS`，including
`-W error::ResourceWarning`.

### 25.2 Explicit exact-only requirement kinds

Coverage protocol is now `task4-compiler-coverage-v2`. Every requirement declares one of five
closed kinds：

```text
TEST_IDENTITY_FUNCTION
FAMILY_FACTORY_FUNCTION
FAMILY_EXECUTOR_FUNCTION
STAGE_ANCHOR_FUNCTION
TRACE_VALIDATION_SUCCESS_FUNCTION
```

The validator compares all of the following exactly：requirement id/kind、stable anchor id、checked
source SHA-256、source-level function、complete compiler mangled function name and the normalized
filename tuple. Compiler paths permit only exact path-component prefix removal；function names use
full equality，never substring matching.

For an expected requirement，an absent/zero function returns zero and fails the family. More than
one function with the same exact compiler identity and source tuple is ambiguous and fails. The
selected LLVM function must contain regions belonging to the one exact checked source-function
range，and both its function count and at least one such region count must be positive. There is no
point、line、sibling-region or enclosing-function fallback.

The exact evidence-test function remains exclusive：only the declared family test identity may be
positive；another or combined test/family report fails before production requirements are credited.

### 25.3 Compiler-visible typed anchors

`task4_runtime_trace.rs` adds eleven private
`#[inline(never)] #[track_caller] task4_coverage_anchor_*` functions. Each existing
`task4_trace_*` method must call its corresponding anchor exactly once；the anchor itself performs
the typed `record(TraceStage::...)` side effect. It is therefore both compiler-visible and
unavoidable on the honest stage path，while the full `track_caller` chain preserves the production
caller file/line used by typed family/instance validation.

After every event/family/instance/order check、serialization and marker emission succeeds，
`verify_and_emit` uniquely calls private `#[inline(never)]
task4_coverage_anchor_validation_success`. Error/early-return paths cannot call it. Static source
fitness rejects missing/duplicate stage and validation anchor definitions or calls；the coverage
manifest independently requires every anchor's exact compiler function count.

Fresh SQLx LLVM inspection showed every one of the 11 stage anchors plus validation-success as one
independent function with count `1`. Default and all-feature reports showed the same property；no
anchor needed a line counter fallback、alternate shape or inlining workaround.

### 25.4 Manifest and seven-family evidence

The v2 coverage manifest has seven ordered family records and `15` exact functions per family
（`105` total）：one exact test identity、one family factory entry、one family executor entry、eleven
typed stage anchors and one typed validation-success anchor. The 29 runtime rows bind the v2 ordered
requirement set；the two DataSource rows remain compile identity with `-` coverage fields.

All seven families first produced fresh isolated reports used to freeze exact compiler identities，
then were independently rerun through the final live verifier. A first DuckDB probe returned
`errno=28` while six completed probe targets were deliberately retained. Only those exact temporary
probe targets were cleaned with `cargo clean` while their JSON reports were retained；about `40 GiB`
was recovered，and DuckDB reran PASS. The workspace target and source were not cleaned. Final live
and semantic runs used automatic per-family TemporaryDirectory cleanup and had no space failure.

Final live output：

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

### 25.5 Fresh gates and hashes

- verifier self/live：`38/38 PASS + 31/21/6/1/runtime7/coverage7/identity2`；
- denominator self/live：`41/41 PASS + Physical 73 / adapter 55 / consumers 3557`；
- semantic self/live：`16/16 PASS + 52/17/UNKNOWN 0`；adapter-live internally reran the complete
  v2 coverage verifier；
- purity self/transitional：`103/103 PASS + STANDARD_FILES 29`；
- wrapper default/all-feature complete test matrices、default/all-feature all-target checks and
  workspace all-target/all-feature check PASS；
- `druid-rdbc` all-target/all-feature `20/20`、doc `1/1` and Clippy `-D warnings` PASS；
- wrapper scoped no-deps Clippy、docs/doctests、Python byte-compile/Black、rustfmt and diff check
  PASS；
- dependency-aware wrapper Clippy remains deliberately non-GREEN only at unchanged Task 5 lines
  `druid_pooled_connection.rs:1479` and `result_set_filter_chain.rs:1309`。

```text
ADAPTER_LIVE_TSV_SHA256=350f6b6350f7c75fbf88aa15872f14faaa7cdb9c90faa0c3d5d03ea8442b6410
RUNTIME_TRACE_TSV_SHA256=106323e17904d377172986ce7464a54de60b4bb575e5f67e333e37f16188a3ec
COVERAGE_FUNCTION_MANIFEST_SHA256=f7b3d4b429fe9e5c4bfe954082f0518d8e78635f6f7cc21bde1ffcd40f2307c9
TRACE_SOURCE_SHA256=4dd93a1fc9f6d23f10659195724ced0f79018528d96b312bdebfa018dc65089e
CONSUMER_LIVE_TSV_SHA256=241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc
SEMANTIC_LIVE_TSV_SHA256=4588212080ef8006cfa9cd6a13427e9867f39bef13b2f985d6350e545352bf21
```

### 25.6 Scope and submission state

The round changes only `task4_runtime_trace.rs`、Task 4 verifier/self-tests、runtime/coverage
manifests and append-only report/ledger. No family adapter/test behavior、Cargo/lock、Task 5、Physical
deletion、admin、unsafe or manual Send/Sync change is included. Previous marker/runner/row issues and
the new exact-region fallback have implementation dispositions of **ADDRESSED, pending independent
review**. Task 5 remains blocked；no push is authorized.
