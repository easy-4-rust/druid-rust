## 1. Migration evidence consistency

- [x] 1.1 Compare the authority TSV with the standardization manifest; repair the `RdbcResultSet` target mismatch and regenerate the derived Markdown table.
- [x] 1.2 Run the authority verifier and its unit tests; record exact output.

## 2. Behavior parity evidence

- [x] 2.1 Run exact method-migration parity, including executable behavior selectors; repair only source-backed mismatches. Evidence: [method-parity.md](evidence/method-parity.md).
- [x] 2.2 Run exact deleted-test parity and the migrated behavior targets; verify current source hashes. Evidence: [deleted-test-parity.md](evidence/deleted-test-parity.md).
- [x] 2.3 Refresh stale compiler-coverage evidence with the approved local generator/toolchain. Evidence: [compiler-coverage.md](evidence/compiler-coverage.md).

## 3. Public API and documentation

- [x] 3.1 Generate the current `druid-rdbc` API and compare the reviewed snapshot; inspect and classify every diff. Evidence: [public-api-diff.md](evidence/public-api-diff.md).
- [ ] 3.2 Run strict rustdoc and SemVer checks; update reviewed evidence only after resolving any findings. Strict Druid rustdoc passes locally. CI run `38630325025` confirmed the two restored XA compatibility modules are no longer SemVer removals; the exact Druid `module_missing` baseline is now 26 findings with SHA-256 `72afa2efd2aafcd842ae8d7220d83e0f1b747a23a4a704d9c3f522b126763431`. The local verifier accepts all 27 approved categories against the uploaded reports; rerun CI after pushing the reviewed allowlist and migration-map updates.

## 4. Build and compatibility acceptance

- [ ] 4.1 Run required workspace tests and lint gates to terminal completion; resolve failures or record a concrete environment blocker. CI run `38630325025` passed the Linux projection-budget test and Windows/macOS workspace tests; the aggregate Rust contract and remaining Apple database matrix jobs are still running. The fixture explicitly allows 4,096 records so the Linux pressure test reaches its byte-budget assertion. Local Windows excludes this `target_os = "linux"` test.
- [ ] 4.2 Run required feature combinations and MSRV checks to terminal completion.
- [ ] 4.3 Validate this OpenSpec change, sync the accepted delta, commit and push the completed Task 6 feature point.
- [ ] 4.4 Mark Task 6 complete only when every preceding required gate has actual passing evidence.
