# Frozen CallableStatement inputs

The seven rows contain all public JUnit test methods from Druid 1.2.28 DruidPooledCallableStatementTest, commit 33824c3dec1612711f9bb4e409319bcab2e4cd0e. Corpus SHA-256: `1c207a287093514fcb7814bd420e84f28620522a5d8b8ad5bb5c040b57021940`. callable_java_sources.tsv pins the test, MockCallableStatement, MockPreparedStatement and DruidPooledCallableStatement sources; these four sources are compiled against the configured frozen build classpath for the live run. This is not an attestation that every Java production class was rebuilt.

Each original assertion method runs unchanged under its actual setup/teardown. A fresh identical setup then executes the explicit SQL, execute flag, filter configuration, original throwError flag, indexed/named String target or Callable/Prepared unwrap role. The helper checks initialSize=1 and reflects the complete seven-method denominator. Both sides record the complete SQL exception payload, before/after error counts, active/idle counts, logical statement/connection close and supplementary physical-holder reuse after return. The additional reborrow probe is symmetric and does not replace the original assertions.

The Rust fixture mirrors only the frozen mock behavior used by these cases: execute returns no current result, initial wasNull is false, and both typed getObject overloads throw SQLFeatureNotSupportedException with code 0 and null SQLState/message. Its real native pooled Callable proxy, statistics policy and wrapper roles produce the observations. The raw allocation address is captured before transferring ownership to distinguish the raw driver from the governed logical view, without unsafe access or a fabricated concrete-class unwrap capability. The fixture is not database-adapter capability evidence.

Configure DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME, DRUID_JAVA_ORACLE_CLASSPATH and DRUID_JAVA_ORACLE_WORK_DIR, using D: or E: for generated/temp evidence, then run:

```sh
cargo test -p druid-bench --locked --test java_rust_callable_differential -- --ignored --nocapture
```

The live comparison has no allowance path. The default Rust corpus test alone does not prove Java parity. Retain actual mismatches for typed-getter error counts and raw/logical wrapper identities pending explicit acceptance decisions.
