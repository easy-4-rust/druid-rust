# Frozen Wall multi-statement inputs

Source: Java Druid 1.2.28, commit `33824c3dec1612711f9bb4e409319bcab2e4cd0e`, `core/src/test/java/com/alibaba/druid/bvt/filter/wall/WallMultiStatementTest.java`.

The source SHA-256 is `ac9ef98f31cfcfa8a1d821cccec657072d2f06f2a72ea617913986d124496fb7`; the LF corpus SHA-256 is `682ab4352eafee9f3f667c3a09d8e8133561e78acc79b38b773bec65f659b8b9`. Both original methods and their exact shared SQL are retained. The Java helper recompiles the original test against the configured frozen build classpath, checks runtime version, reflects the complete two-method denominator and original private SQL, and runs each original assertion before recording its decision. Rust dispatches the same corpus to the corresponding WallUtils dialect. The live comparison permits no differences.

Configure `DRUID_JAVA_ORACLE_ROOT`, `DRUID_JAVA_ORACLE_JAVA_HOME`, `DRUID_JAVA_ORACLE_CLASSPATH` (the frozen build's Surefire java.class.path) and `DRUID_JAVA_ORACLE_WORK_DIR`, then run:

```sh
cargo test -p druid-bench --locked --test java_rust_wall_multi_differential -- --ignored --nocapture
```

All local work, evidence and temporary directories must be on D: or E:. The ordinary default test verifies both Rust decisions; its ignored live test is not evidence of Java parity until explicitly executed. This corpus proves only these two multi-statement rejection decisions, not dialect-wide SQL security, Wall table statistics or the complete P07 semantic gate. The helper recompiles the pinned original test, not every Java production class; configured build provenance remains part of the frozen oracle's external precondition.
