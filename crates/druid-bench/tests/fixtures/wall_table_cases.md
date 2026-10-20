# Frozen WallFilter H2 table-statistics case

The 20 operation rows describe the entire single public JUnit method WallFilterTest#test_wallFilter in frozen Java Druid 1.2.28, commit 33824c3dec1612711f9bb4e409319bcab2e4cd0e. Original source SHA-256: 269698f1122383bb22fa8b58df06770d0d29e938319b9a88aa885d6a1ebb2569. Corpus SHA-256: 0494e11cb12bc5bc9e139fbd861fc5fcdf7ce8fd09008fc192f3dbbb03367dbf.

The Java helper checks version, source/corpus identities and the complete one-method denominator. It runs the unchanged original JUnit assertion under actual setup/teardown, then executes the corpus on a fresh original setup. The original test and helper are compiled against the frozen build classpath; this does not attest a full rebuild of every Java production class.

Both observations retain the original SQL strings, two integer parameters, ten inserts on one borrowed connection, a prepared SELECT consumed through next(), two separately borrowed prepared generic updates, a two-entry prepared batch, TRUNCATE and DROP. Each remaining step closes its statement and connection before observing the corresponding table counter. The output contains all ten original checkpoints in order; final totals alone cannot replace intermediate observations.

Rust executes through the real Druid pool, WallFilter and JDBC Agent. No simulated driver, manual provider counter increments, SQLite substitution or rewritten SQL is used. Both sides must use the same H2 driver JAR; the retained local run uses the frozen Java build's H2 1.4.199, not a mixed H2 version comparison. The Agent JAR and H2 JAR digests accompany source hashes. This comparison is specific to that H2 version and is not certification of every JDBC adapter or database version.

Set DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME, DRUID_JAVA_ORACLE_CLASSPATH, DRUID_JAVA_ORACLE_WORK_DIR, DRUID_JDBC_AGENT_JAR and DRUID_JDBC_DRIVER_JAR to the frozen Java build, current Agent and matching driver, with D:/E: output/temp locations. Run:

```sh
cargo test -p druid-bench --locked --features jdbc-agent --test java_rust_wall_table_differential -- --ignored --nocapture
```

The bench jdbc-agent feature is opt-in. Its default corpus test does not execute the environment-dependent oracle; the explicitly ignored live test is the paired evidence and fails on any unapproved difference.
