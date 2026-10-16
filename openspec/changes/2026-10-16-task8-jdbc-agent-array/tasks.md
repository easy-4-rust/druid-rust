# Tasks

- [x] 1. Add a failing H2 test for Array registration, explicit free, retry, and logical/session close; implement AgentSession ownership and rerun it.
- [x] 2a. Add Agent JSON-RPC Array create, metadata, values, 1-based range and free operations; verify SQL NULL, null element-array forwarding, session isolation, and precise unsupported type-map error against H2.
- [x] 2b. Add Agent Array result-set cursor operations, owner cleanup, and H2 protocol tests.
- [x] 2c. Preserve byte, short, integer, long, float and double Java wrapper identity across the Rust/Java Array element protocol; verify H2 INTEGER metadata and roundtrip.
- [ ] 3. Implement the Rust JDBC Agent standard Array adapter with truthful capability bits and pooled owner lifecycle.
- [ ] 4. Run Java and Rust real-H2 tests, resource contract regressions, and Task 8 semantic verifier; update only proven inventory states.
- [ ] 5. Validate OpenSpec, review compatibility and resource failure behavior, then sync/archive after complete implementation.
