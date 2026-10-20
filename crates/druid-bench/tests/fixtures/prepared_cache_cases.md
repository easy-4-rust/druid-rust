# Frozen prepared-cache case

The single corpus row covers the complete public JUnit denominator of Java Druid 1.2.28 DruidPooledPreparedStatementTest at commit 33824c3dec1612711f9bb4e409319bcab2e4cd0e. Source SHA-256: a5cc279eab7207b19c3cb98ff0a9c85cafa1e5db9ef7f5dc2ab91d976b62ce9a. Corpus SHA-256: 7681fa4dac8ffdb02343fb72a5868f39a5f5df3b7c66f5590f7a9d27c26f2d10.

The live helper checks version, source/corpus identities and the one-method denominator. It runs the unchanged original assertion under its actual setup/teardown, then starts a fresh identical setup to record both borrows. Java source is compiled against the frozen build classpath; this does not attest a full rebuild of all Java production classes.

Both sides use the original select ?, parameter xx, stat filter, prepared caching, test-on-borrow=false and five property values 1/2/3/4/5. The mock driver preserves raw direction integers 0 and 2 via FetchDirection::DriverValue, without replacing them with standard constants or weakening TryFrom integer validation. Its query returns one row as asserted by the original test; cell values are not asserted or compared here. Real Rust pooled proxies perform property restoration, implicit statement cleanup on connection return and cache reuse.

The result retains property snapshots, row traversal, SQL, both original statement hit counts and the second key-presence assertion. Rust standard PreparedStatement does not expose the Java Druid-specific getHitCount/getKey diagnostics, so these fields read NOT_EXPOSED. Supplementary pool-level hit/active/idle counters are observed separately on both sides and cannot stand in for those fields. The pre-release method migration ledger already records removal of DruidPooledPreparedStatement::key, but no new behavioral waiver is inferred from that ledger. The live test fails until the exact acceptance gap is resolved.

Set DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME, DRUID_JAVA_ORACLE_CLASSPATH and DRUID_JAVA_ORACLE_WORK_DIR to the frozen build and D:/E: output paths, then run:

```sh
cargo test -p druid-bench --locked --test java_rust_prepared_cache_differential -- --ignored --nocapture
```

Default module tests skip the environment-dependent Java run. They verify observation coverage without claiming cross-language equivalence.
