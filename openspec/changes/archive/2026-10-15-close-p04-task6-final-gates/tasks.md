## 1. Migration evidence consistency

- [x] 1.1 Compare the authority TSV with the standardization manifest; repair the `RdbcResultSet` target mismatch and regenerate the derived Markdown table.
- [x] 1.2 Run the authority verifier and its unit tests; record exact output.

## 2. Behavior parity evidence

- [x] 2.1 Run exact method-migration parity, including executable behavior selectors; repair only source-backed mismatches. Evidence: [method-parity.md](evidence/method-parity.md).
- [x] 2.2 Run exact deleted-test parity and the migrated behavior targets; verify current source hashes. Evidence: [deleted-test-parity.md](evidence/deleted-test-parity.md).
- [x] 2.3 Refresh stale compiler-coverage evidence with the approved local generator/toolchain. Evidence: [compiler-coverage.md](evidence/compiler-coverage.md).

## 3. Public API and documentation

- [x] 3.1 Generate the current `druid-rdbc` API and compare the reviewed snapshot; inspect and classify every diff. Evidence: [public-api-diff.md](evidence/public-api-diff.md).
- [x] 3.2 Run strict rustdoc and SemVer checks; update reviewed evidence only after resolving any findings. Strict Druid rustdoc passes with `RUSTDOCFLAGS=-D warnings`; CI run `38634035936` passed `semver` and `public-api`. The exact Druid `module_missing` baseline is 26 findings with SHA-256 `72afa2efd2aafcd842ae8d7220d83e0f1b747a23a4a704d9c3f522b126763431`; the local verifier accepts all 27 approved categories. `verify_task6_public_contract.py` also passed with 128 allowlist rows and the current Druid/Wrapper API hashes.

## 4. Build and compatibility acceptance

- [x] 4.1 Run required workspace tests and lint gates to terminal completion; resolve failures or record a concrete environment blocker. CI run `38634035936` passed `rust-contracts`, `Rust contracts (ubuntu-latest)`, `Rust contracts (macos-latest)`, and `Rust contracts (windows-latest)`, including workspace tests, all-feature tests, and clippy. The Linux projection-budget test passed after its fixture explicitly allowed 4,096 records.
- [x] 4.2 Run required feature combinations and MSRV checks to terminal completion. CI run `38634035936` passed `msrv`, the no-default/isolated feature checks, and the wrapper/catalog dependency-boundary steps in `rust-contracts`.
- [x] 4.3 Validate this OpenSpec change, sync the accepted delta, commit and push the completed Task 6 feature point. Strict validation, archive/spec sync, feature commit, and push are performed as this finalization sequence.
- [x] 4.4 Mark Task 6 complete only when every preceding required gate has actual passing evidence. The P04 Task 6-specific API, SemVer, rustdoc, workspace, feature, and MSRV gates all have passing evidence; broader real-database matrix coverage remains tracked outside Task 6.
