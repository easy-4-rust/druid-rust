# Tasks

- [x] 1. Add a failing H2 test for Array registration, explicit free, retry, and logical/session close; implement AgentSession ownership and rerun it.
- [x] 2a. Add Agent JSON-RPC Array create, metadata, values, 1-based range and free operations; verify SQL NULL, null element-array forwarding, session isolation, and precise unsupported type-map error against H2.
- [x] 2b. Add Agent Array result-set cursor operations, owner cleanup, and H2 protocol tests.
- [x] 2c. Preserve byte, short, integer, long, float and double Java wrapper identity across the Rust/Java Array element protocol; verify H2 INTEGER metadata and roundtrip.
- [x] 3a. Connect `Connection#create_array_of` and the Rust Array metadata, value/range, free and retry methods through the owned executor; verify with fake transport and real H2, while advertising only READ/RANGE/FREE.
- [x] 3b1. Expose a forward-only Array ResultSet through the owned executor with bounded 8-row pages, typed values, metadata and remote close; verify the page boundary and parent `free()` invalidation against real H2 before advertising RESULT_SET.
- [x] 3b2. Verify a 503-element Array over multiple bounded pages in one real-H2 Agent session; two consecutive runs passed. The earlier parallel-test timeout came from shared Agent runtime ownership.
- [x] 3b3. Verify a real-H2 pooled owner close returns its holder and invalidates an outstanding Array and child cursor; run the direct and pooled H2 tests together with separate Agent runtime identities.
- [x] 3b4. Verify `ResultSet#next` cancellation while a remote page is pending: a controlled executor pauses the page, the caller cancels `next`, and a later `next` resumes the same request without replay before closing the cursor.
- [ ] 4. Run Java and Rust real-H2 tests, resource contract regressions, and Task 8 semantic verifier; update only proven inventory states.
- [ ] 5. Validate OpenSpec, review compatibility and resource failure behavior, then sync/archive after complete implementation.
