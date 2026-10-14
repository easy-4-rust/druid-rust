# Final delivery checks (tasks 9.1–9.3)

Date: 2026-10-14. Repository: `druid-rust`, branch `main`. Toolchain: Cargo 1.97.1, rustc 1.97.1, OpenSpec 1.14.0. All Cargo runs below used the checked-in lockfile and offline mode.

## 9.1 Final regression and purity

| Command | Result | Notes |
|---|---|---|
| `cargo test -p druid-rdbc --locked --offline` | PASS (exit 0) | Full crate, integration targets, and doctests. |
| `cargo test -p druid --locked --offline` | PASS (exit 0) | Full crate, integration targets, and doctests; external consumer parity target passed 3/3. |
| `cargo test -p druid-wrapper --locked --offline --features "sqlx toasty-sqlite jdbc-agent"` | PASS (exit 0) | Full wrapper feature integration and doctests. JDBC Agent XA external consumer target separately passed 2; 3 live Java/PostgreSQL cases remained ignored because their documented external services/classpaths were not configured. |
| `python -m unittest discover -s scripts -p test_verify_rdbc_single_standard_purity.py -v` | PASS, 112 tests | Includes macro-shadow and dependency-alias negative controls. |
| `python scripts/verify_rdbc_single_standard_purity.py --phase final` | PASS | `STANDARD_FILES=128`, `PURITY_PHASE=final`. |
| `python scripts/verify_java8_method_evidence.py` | PASS | `inventory=2001`, `ledger=2001`, `linked=9`, `unassessed=1992`, `not_run=0`, `failed_layers=0`; evidence gate confirms recorded source/test hashes. |
| `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` | PASS | Strict change validation. |
| `cargo fmt -p druid -p druid-wrapper -- --check` | PASS | Formatting check for both affected crates. Workspace-wide `cargo fmt --all -- --check` could not enumerate a path because Windows returned OS error 206; the changed Rust files are covered by the crate-scoped check. |

The full `druid` test run exposed a stale test expectation: DriverManager's no-suitable-driver contract is SQLState `08001`, while the assertion expected the internal `DriverError` variant. The assertion now checks the public SQL exception contract. The wrapper run likewise exposed a stale generated-key expectation: unsupported column selection is a feature-not-supported SQL exception (`0A000`), not the internal `UnsupportedOperation` variant. The corrected test also verifies no insert and no prepared-cache entry. Both targeted tests and their complete crate suites pass.

Final changed-source SHA-256 identities:

| File | SHA-256 |
|---|---|
| `crates/druid/tests/standard_driver_action_test.rs` | `DFDA7AD76A240CEE61C4B9199A04427EBF4DAE6C6424951D11CB53CE22E57A1B` |
| `crates/druid-wrapper/tests/generated_keys_prepare_test.rs` | `97B5008E0E270CE99B42A32EF52DFE8DB65FCA186F871319C8BDE62D526C7C87` |
| `scripts/verify_rdbc_single_standard_purity.py` | `92DE0BE05F8B417EFE3A7E0C415275EA97D8D3524637CEF44DFA0CE65797766C` |
| `scripts/test_verify_rdbc_single_standard_purity.py` | `CEE8CF0F6EE41361B20348A79CB0D98CC5EEA77CEFAECD73A22CE917726C6544` |

## 9.2 Public API, SemVer, and external consumers

The change's API and migration decisions remain represented by the reviewed migration evidence and the explicit public-contract allowlist; no allowlist was broadened during final validation. Consumer checks passed:

| Consumer boundary | Command | Result |
|---|---|---|
| RDBC XA contract / transaction-manager consumer | `cargo test -p druid-rdbc --locked --offline --test xa_standard_external_consumer` | 4 passed |
| Druid pool consumer | `cargo test -p druid --locked --offline --test task6_rdbc_external_consumer_test_parity` | 3 passed |
| JDBC Agent XA consumer | `cargo test -p druid-wrapper --locked --offline --features jdbc-agent --test jdbc_agent_xa_external_consumer` | 2 passed, 3 environment-gated live cases ignored |

The SemVer report generator requires three pinned `cargo-semver-checks` reports and an explicit reviewed allowlist. No fresh cross-version SemVer reports were produced in this final pass, so this evidence records the source/migration review and consumer behavior; it does not claim a newly generated compiler SemVer report. The external Java Agent/PostgreSQL recovery cases remain environment-gated and are separately recorded as such in the XA evidence.

## 9.3 Temporal, cancellation, XA, and architecture evidence review

- The Java 8 temporal differential record remains the source of truth for timezone, DST, and epoch behavior. `verify_java8_method_evidence.py` passed against the final repository and verifies source and evidence hashes; the only code changes in this final pass are test/verifier corrections described above and do not alter temporal implementation.
- Cancellation evidence and the SQLx cancellation capability matrix remain explicit about live backend runs versus `NOT_RUN`; this pass did not promote any backend capability. The complete wrapper regression, including cancellation/cursor lifecycle tests, passed.
- XA standard contract and Rust client consumer tests passed. The JDBC Agent/PostgreSQL live recovery tests remained ignored because the Java Agent and PostgreSQL Testcontainers environment was not configured; existing evidence retains this limitation.
- The checked-in Chinese architecture diagram and resource/result-set capability matrix were reviewed against their evidence ledgers. Real database Array/LOB/SQLXML/SQLData/RowSet capability checks remain `NOT_RUN` as recorded in the 8.8 ledger.

## Out-of-scope verifier observations

Two standalone checks were also probed and are not treated as acceptance for this change: the final semantic-inventory phase reports pre-existing `PLANNED` rows in the global 2,001-entry method inventory, and the wrapper-import boundary checker reports `druid_rdbc::driver::ConnectionFactory` as unclassified. Neither is hidden or promoted to PASS here; both require a separately scoped inventory/boundary update. The change-specific strict OpenSpec validation, complete crate regressions, final standard-purity gate, and method-evidence freshness gate are green.

## 9.4 Delivery boundary and closure

All 56 tasks in this OpenSpec change have completion evidence and the change's strict validation is green. Seven incremental capability specifications are synchronized by the OpenSpec archive operation. This closes only `close-rdbc-jdbc42-contract-gaps`.

The delivery does not claim P04–P07 completion, full JDBC 4.2 coverage, TCK certification, all-backend resource support, or project-wide production readiness. Backend-specific Array/LOB/SQLXML/SQLData/RowSet checks remain `NOT_RUN`; three JDBC Agent live XA cases remain environment-gated; a fresh SemVer report was not generated in this final pass. The global semantic inventory and wrapper-import-boundary observations above remain follow-up work outside this change's acceptance boundary.
