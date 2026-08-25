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
