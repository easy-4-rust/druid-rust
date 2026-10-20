# Frozen Pool acquisition and recycle inputs

The corpus contains every public JUnit test method from Druid 1.2.28's DruidDataSourceTest_getConnection (3), DruidDataSourceTest_recycle (1), and DruidDataSourceTest_recycle2 (2), commit 33824c3dec1612711f9bb4e409319bcab2e4cd0e. The non-test f_test_recycle_error_interrupt method remains outside this JUnit selection. Source files and hashes are in pool_java_sources.tsv. Corpus SHA-256: `09dd147f9f41fa33a9950f67b23d3d21d54819ed873d61f81c11f8058a1b596d`.

Each Java case runs the original unchanged assertion method under its actual setup/teardown, then a fresh identical setup supplies the explicit observations. The helper reflects the complete six-method denominator and checks maximum-active and filter configuration. A dash in the fixed corpus represents Java NULL (credentials) or an operation absent from the original case (SQL); an empty string is not substituted for NULL. Both sides execute select 1 for the recycle cases, retain transaction and rollback-filter inputs, and sample counters before and after close. Both perform a supplementary reborrow after successful close to check physical-holder reuse and final active/idle counts. Statement closure is sampled after logical connection close, including the discarded-connection branch, and differences are retained.

Configure DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME, DRUID_JAVA_ORACLE_CLASSPATH and DRUID_JAVA_ORACLE_WORK_DIR as for the other frozen comparators. Run:

```sh
cargo test -p druid-bench --locked --test java_rust_pool_differential -- --ignored --nocapture
```

Use D: or E: for work and temporary evidence. The Java helper recompiles the three pinned original tests against the configured frozen production build; it does not rebuild or attest every production Java class. Rust uses the actual native DruidDataSource/pool over the existing tracking driver fixture. This proves proxy/lifecycle behavior, not a real database adapter capability. The default Rust test executes all six inputs but ignores the environment-dependent Java test. A real live comparison is required, and has no allowance path; unapproved differences remain failed.
