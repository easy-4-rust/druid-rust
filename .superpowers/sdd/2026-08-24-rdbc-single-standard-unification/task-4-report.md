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
