# Tasks

## 1. Session identity and ownership

- [x] 1.1 Add Java tests that fail before `createNClob` routing and prove the driver method, distinct resource identity, retryable failed free, child cleanup, and logical/session close; implement the Agent path and pass the focused tests. The new tests first failed on missing session methods and the missing protocol route, then passed; JDK 17 `mvn verify` passed, including the PostgreSQL Testcontainers suite.

## 2. Standard adapter and real drivers

- [x] 2.1 Add a failing external RDBC consumer for `create_n_clob` against H2 and HSQLDB; implement the dedicated NClob adapter with inherited operations. Verify H2 2.3.232 search/truncation error fidelity (`HYC00`/`50100`) and HSQLDB 2.7.4 successful search/truncation, retaining both drivers' tests. The external test first failed with standard `0A000` before the adapter existed; after implementation, both real-driver tests passed.
- [x] 2.2 Add more-than-one-page UTF-16 and ASCII stream, cancellation, free, and pooled-owner tests for both drivers; implement only missing NClob behavior and pass those tests without changing the Clob contract. Four H2/HSQLDB Rust external-consumer tests passed: both drivers transferred 100,000 UTF-16 units and invalidated children on free/owner close; HSQLDB transferred 160 KiB ASCII, while H2 preserved its `HYC00`/`50100` ASCII-output rejection. The NClob cancellation session test passed under JDK 17.

## 3. Evidence and integration

- [x] 3.1 Advance only the source-bound NClob inventory row after the live adapter tests pass; verify the semantic inventory, coverage/source identity, and verifier self-tests. The adapter-live semantic gate passed with `java.sql=52`, `javax.sql=17`, `UNKNOWN=0`, frozen SHA-256 `afe4824599b8843f769440afa5c805a822985b9fb8d9ef46e7f8739bf7fa5c26`, and live SHA-256 `4b271b2d3ec98784c82859e2dcb014cdbd2b26af7b34e34dba2c2f0a988a7fc4`; 112 semantic, standard, and wrapper verifier tests passed locally (one platform skip).
- [ ] 3.2 Run Java verification, Rust resource and wrapper regressions, formatter, Clippy, API/SemVer, OpenSpec strict validation, and the H2/HSQLDB cross-platform CI matrix; record the actual results, then sync and archive the change.
