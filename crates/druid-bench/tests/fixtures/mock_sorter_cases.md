# Frozen Mock exception-sorter case

The single row is the complete public JUnit denominator of Java Druid 1.2.28 ExceptionSorterTest at commit 33824c3dec1612711f9bb4e409319bcab2e4cd0e. The comparator checks the original source SHA-256 cf1665f28399e9bb17d9e791651d4f5a3941bfe54c29cc5d93e1a7385cabedfd and corpus SHA-256 69faf3454701669a2dc6a2095285a73684c8298364ed1615b05aedf09eefb952. It compiles that original test and the observation helper against the configured frozen Java build classpath; this does not attest a full rebuild of every Java production class.

The helper executes the unchanged original assertion with its actual setup/teardown. A fresh original setup then records the selected sorter, full SQL error payload and discard/active/idle counts after physical connection close, statement close and logical connection close. Rust uses the real native pool with Mock driver selection, mysql database type, prepared caching, test-on-borrow and the stat filter enabled. The shared test driver models the physical-close cleanup failure; Java MockConnectionClosedException() uses null message and SQLState, which the fixture must preserve. This evidence concerns the mock cleanup path, not real database capability or all vendor sorters.

Run the default druid-bench module tests, then explicitly execute:

```sh
cargo test -p druid-bench --locked --test java_rust_mock_sorter_differential -- --ignored --nocapture
```

Set DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME, DRUID_JAVA_ORACLE_CLASSPATH and DRUID_JAVA_ORACLE_WORK_DIR to the frozen Java build and D:/E: output locations. Default ignored execution is not Java evidence. The live comparator has no allowance path and fails on any unequal field.
