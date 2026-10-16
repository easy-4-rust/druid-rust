# Tasks

## 1. Session ownership

- [ ] 1.1 Add a failing Java test for Blob registration, explicit free, failed-free retry, and logical/session close; implement the registry and pass the test.
- [ ] 1.2 Add child stream ownership and close-order fault tests; implement aggregated cleanup and pass the Java tests.

## 2. Driver byte operations

- [ ] 2.1 Add real-H2 protocol tests for length, one-based reads/search, both byte-write overloads, truncation, and original SQL errors; implement the requests and pass `mvn verify`.
- [ ] 2.2 Add a negative capability/error test for unsupported Blob operations, then make protocol advertisement and `0A000` behavior match it.

## 3. Bounded streams

- [ ] 3.1 Add a greater-than-one-page H2 input-stream test and cancellation/close assertions; implement bounded remote reads and pass `mvn verify`.
- [ ] 3.2 Add a greater-than-one-page H2 output-stream test with write, flush, close, and parent-free behavior; implement bounded remote writes and pass `mvn verify`.

## 4. Standard Rust adapter

- [ ] 4.1 Add failing external-consumer tests for Blob factory, values, mutation, search, failure/retry, and capability bits; implement through the owned executor and pass focused Rust tests against H2.
- [ ] 4.2 Add external-consumer stream and pooled-owner invalidation tests; implement the bounded stream bridge and pass H2 plus RDBC resource regressions.

## 5. Integration

- [ ] 5.1 Run Java, Rust H2, resource, semantic, API, and SemVer checks; advance only proven Blob inventory state and verify the frozen source oracle remains valid.
- [ ] 5.2 Review cross-platform CI and compatibility, run strict OpenSpec validation, then sync and archive only when all requirements have evidence.
