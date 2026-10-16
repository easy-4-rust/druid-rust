# Tasks

## 1. Session ownership

- [x] 1.1 Add a failing Java test for Blob registration, explicit free, failed-free retry, and logical/session close; implement the registry and pass the test. Missing-method compilation failed first; focused H2 3/3 and JDK 17 `mvn verify` passed.
- [x] 1.2 Add child stream ownership and close-order fault tests; implement aggregated cleanup and pass the Java tests. The missing-method test failed first; H2 target 4/4 and JDK 17 `mvn verify` passed.

## 2. Driver byte operations

- [x] 2.1 Add real-H2 protocol tests for length, one-based reads, both byte-write overloads, and original `HYC00/50100` search/truncation errors; add real-HSQLDB successful one-based search and truncation tests; implement the requests and pass `mvn verify` plus focused Rust-to-Agent tests for both drivers. Java AgentServer H2 and HSQLDB protocol tests passed under JDK 17 `mvn verify`; Rust external-consumer H2 4/4 and HSQLDB 1/1 passed against the packaged Agent.
- [x] 2.2 Add a negative capability/error test for unsupported Blob operations, then make protocol advertisement and `0A000` behavior match it. The protocol test first failed with a null SQLState, then passed with `SQLFeatureNotSupportedException/0A000` for `position(Blob,long)` and no matching ready/handshake capability; JDK 17 `mvn verify` passed.

## 3. Bounded streams

- [x] 3.1 Add a greater-than-one-page H2 input-stream test and cancellation/close assertions; implement bounded remote reads and pass `mvn verify`. Real H2 reads 160 KiB through bounded pages, rejects a 65,537-byte request, preserves ranged reads and EOF, and invalidates closed/parent-freed streams; a blocking child-stream fixture proves concurrent JSON-RPC cancel closes the handle and returns `HY008`. JDK 17 `mvn verify` passed.
- [x] 3.2 Add a greater-than-one-page H2 output-stream test with write, flush, close, and parent-free behavior; implement bounded remote writes and pass `mvn verify`. The missing protocol first failed; H2 then passed five 32-KiB writes, the 64-KiB request bound, flush, close, parent free with an open child, and JDK 17 `mvn verify`.

## 4. Standard Rust adapter

- [x] 4.1 Add failing external-consumer tests for Blob factory, values, mutation, search, failure/retry, and capability bits; implement through the owned executor and pass focused Rust tests against H2 error behavior and HSQLDB positive search/truncation. Real H2 external consumer 4/4 verifies factory, values, mutations, original SQL errors, recovery after a nonfatal error, conservative capability bits, and pooled owner; real HSQLDB 1/1 verifies successful search/truncation. The owned-executor failed-free fixture verifies handle retention and retry. Strict wrapper Clippy and direct rustfmt passed.
- [x] 4.2 Add external-consumer stream and pooled-owner invalidation tests; implement the bounded stream bridge and pass H2 plus RDBC resource regressions. Real H2 external-consumer 4/4 covers 160-KiB paged input/output, ranged read, flush/close, parent and pooled-owner invalidation, and current-thread fail-fast. `rdbc_blob_test` 2/2, RDBC resource contracts 32/32, resource-owner cancellation 118/118, strict wrapper Clippy, and direct rustfmt passed. Legacy output-stream flush-failure closure semantics remain unchanged; remote close failures retain the handle for retry.

## 5. Integration

- [x] 5.1 Run Java, Rust H2, resource, semantic, API, and SemVer checks; advance only proven Blob inventory state and verify the frozen source oracle remains valid. JDK 17 Maven verify, H2 4/4, HSQLDB 1/1, resource contracts, semantic logical-live, 31 wrapper adapters, public-API and SemVer CI gates passed; frozen source SHA stayed fe4824599b8843f769440afa5c805a822985b9fb8d9ef46e7f8739bf7fa5c26.
- [x] 5.2 Review cross-platform CI and compatibility, run strict OpenSpec validation, then sync and archive only when all requirements have evidence. driver-matrix run 38852545260 completed successfully across its platform/driver jobs; openspec validate 2026-10-16-task8-jdbc-agent-blob --strict passed.
