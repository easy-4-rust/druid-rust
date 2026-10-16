# Tasks

- [ ] 1. Add failing Java Agent protocol tests for callable preparation, indexed/named registration, execution, OUT read/null, invalid selector, cross-session handle and close/cancel cleanup.
- [ ] 2. Implement session-owned Java callable operations and truthful capability discovery; run Java unit tests against H2 and HSQLDB.
- [ ] 3. Add a failing Rust external-consumer test through `JdbcAgentConnection::prepare_call` and Druid's pooled path, including failure and reuse.
- [ ] 4. Implement the Rust callable adapter and ordered wire descriptors without a second runtime, preserving result sequence, error detail and resource ownership.
- [ ] 5. Run real H2/HSQLDB end-to-end tests, affected Rust regressions, strict formatting/Clippy, semantic inventory, API/SemVer and cross-platform CI; record exact evidence and remaining unsupported methods.
- [ ] 6. Sync and archive only after acceptance passes, then commit and push each completed feature point.
