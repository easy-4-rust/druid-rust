# JDBC ClientInfo overloads and lease cleanup — 2026-10-21

OpenSpec fact source: `rdbc-jdbc-method-conformance/spec.md#MC-09`, under the complete Rust JDBC requirement MC-08. The standard Connection now exposes `client_info_properties` and `set_client_info_properties`, covering Java's no-argument getter and Properties setter. The default whole-set algorithm replaces properties, deletes omitted names, clears an empty set, distinguishes NULL input, and returns independent snapshots. Unsupported snapshots remain explicit errors rather than fabricated empty success.

`RdbcError::ClientInfoException` retains failed-property statuses, original cause and SQL diagnostics. Earlier successful writes remain observable after partial failure. Only the standard sequential algorithm adds unknown status for unattempted operations; pooled/external proxies preserve a driver's complete failure map without adding successful properties.

Druid snapshots client info before first mutation and marks the cleanup obligation before awaiting the driver. Explicit close restores the baseline before reuse. Unknown baseline, failed restoration or dirty Drop causes discard. Native pooled owners keep logical-lease properties isolated; external connection leases forward both complete overloads and classify setter failures. The external lease alone does not claim to reset an independently managed external pool.

## Actual verification

- Unified module command: `cargo +1.97.1 test -p druid-rdbc -p druid --locked --no-fail-fast`. Terminal exit **101**: **3,668 passed, 6 failed, zero ignored**, 242 result blocks. All eleven new ClientInfo cases passed. The six failures were obsolete generic DriverError assertions in three existing Druid scalar-conversion targets after the preceding numeric/scalar implementation changes.
- Only assertions were corrected in those three targets, retaining exact SQLDataException/SQLState checks, original-driver-error priority, exception counts and value/stream observations. No production source changed after the unified run began.
- Closure: `cargo +1.97.1 test -p druid --locked --lib --test task6_complete_row_set_contract_test --test task6_typed_object_default_contract_test --no-fail-fast`, exit **0**: **176 passed, zero failed/ignored**. This replaces the three failed target results; it is not a fabricated single clean full-module run. Across the full run plus targeted closure, all 3,674 unique default-module cases have passing observations on the frozen production implementation.
- Final strict all-target RDBC/Druid Clippy: `cargo +1.97.1 clippy -p druid-rdbc -p druid --locked --all-targets -- -D warnings -A clippy::pedantic`, exit **0**. The preceding Clippy log retains a test-only err_expect lint failure, corrected with explicit Result matching.
- Thirteen evidence-control tests passed. The method ledger integrity check and strict OpenSpec validation passed. Both newly mapped Properties overloads remain PARTIAL: declaration and standard-contract evidence only. Inventory: **2,001 identities; 23 PARTIAL links; 1,978 unassessed**. No adapter, complete logical-proxy method conformance or real-database layer is promoted by these two links.

## Scope still open

Native SQLx/Toasty/RBDC/JDBC Agent client-info implementations and real-database behavior are not certified by the stateful Rust contract driver. Public additions require generated native Darwin API snapshot review; the existing snapshots predate this additive surface. Current whole-workspace/platform, SemVer and remaining JDBC interface/behavior gates remain open. Overall verdict: **NOT_READY**; this is not 100% JDBC completion or production readiness.

The first compilation run (`jdbc-client-info-modules-20261021.log`, exit -1) was deliberately stopped for a genuine failed-property-map review correction before completion. It is not passing test evidence. No old failure log was overwritten.

## Raw local receipts

All files below are plain logs in `D:/druid-rust-tooling/`; SHA-256 binds their actual bytes.

| Log | SHA-256 |
| --- | --- |
| `jdbc-client-info-modules-final-20261021.log` | `4cc87878f53ca2ca9734a37bcfd5565daa13d827f7a06f790919caa975a24b7c` |
| `jdbc-client-info-scalar-closure-20261021.log` | `c49f17db63f8043a74fa7001d7de4686944188350f6138ba1e614133f1439e47` |
| `jdbc-client-info-clippy-closure-20261021.log` | `a50e0ddbd62795bafba5eab7672a8d91e219dff74877370872dcf9dd78a23a85` |
| `jdbc-client-info-evidence-controls-20261021.log` | `28d33ab9e887418d505f7dc6a75a12537f63db79e2fc600914ab070ff905621a` |

Standard references: [Connection](https://docs.oracle.com/javase/8/docs/api/java/sql/Connection.html), [SQLClientInfoException](https://docs.oracle.com/javase/8/docs/api/java/sql/SQLClientInfoException.html).
