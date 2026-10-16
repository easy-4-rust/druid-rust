# Tasks

- [x] 1. Write failing session tests for Clob register/free, failed-free retry, child cleanup, and logical/session close; implement owned handles and pass Java tests. Missing-method compilation failed first; AgentClobSessionTest and JDK 17 mvn verify passed, including four child stream types, aggregated close failures, and XA logical close.
- [ ] 2. Write real-driver protocol tests for UTF-16 length, one-based range, both string setters, search, truncation, unsupported behavior and original SQL errors; implement non-stream requests.
- [ ] 3. Write greater-than-one-page Reader/Writer and ASCII stream tests with cancellation, EOF, close, and parent invalidation; implement bounded protocol and bridges.
- [ ] 4. Write external standard RDBC consumer and pooled-owner tests against H2 and HSQLDB; implement the adapter, verify capability bits, and retain nonfatal-error recovery.
- [ ] 5. Run Java, Rust resource, semantic, API, SemVer, and cross-platform CI checks; advance only proven Clob inventory state, validate OpenSpec, sync and archive when complete.
