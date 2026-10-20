# Frozen Java/Rust exception-sorter comparison

This is a live comparison of the 576 inputs in `exception_sorters.tsv` across
MySQL, PostgreSQL, DB2, Informix, Sybase, Oracle, OceanBase Oracle, Null and Phoenix
classifiers. It is not a claim that every Java Druid test or database product has
been certified. Mock driver lifecycle has a separate pool-level oracle.

`exception_sorter_java_sources.tsv` binds eleven original source files to Java
Druid 1.2.28 commit `33824c3dec1612711f9bb4e409319bcab2e4cd0e`. The test checks
their SHA-256 values and recompiles them before running; source drift fails.
The classpath supplies that baseline's logging/support dependencies. Oracle
custom error-code sets are reset before applying the case's explicit properties.

Set these environment variables before the explicit run:

| Variable | Value |
| --- | --- |
| `DRUID_JAVA_ORACLE_ROOT` | Directory containing the original frozen Java source bytes |
| `DRUID_JAVA_ORACLE_CLASSPATH` | Compiled dependency classpath from the frozen Java build |
| `DRUID_JAVA_ORACLE_JAVA_HOME` | JDK containing both `javac` and `java` |
| `DRUID_JAVA_ORACLE_WORK_DIR` | Existing writable directory for retained evidence |

```text
cargo test -p druid-bench --locked --test java_rust_exception_sorters_differential -- --ignored --nocapture
```

The explicit test fails when configuration is missing. The normal workspace run
reports it as ignored, which is not differential evidence. Output directories
retain compiler diagnostics, original Java stdout/stderr, UTF-8 per-case Java
results, and `comparison.tsv`. Runtime exception categories and Rust panics are
observable outcomes. Every case must be present exactly once on each side;
unexpected differences fail and are retained in the report.

The original Java cases are executed without suppressing their diagnostic logs.
Structured results use a separate UTF-8 file so a host's log encoding cannot
corrupt the case protocol. `-` denotes null/missing in the input columns; an
empty message or SQLState field remains an empty string.

The initial complete live run on JDK 17.0.12 recorded 573 MATCH and 3 MISMATCH:
`oracle_null_message`, `oceanbase_oracle_null_message`, and
`phoenix_null_message`. Each has code 0, no SQLState and a null message. Java
throws `java.lang.NullPointerException`; Rust returns false. These cases remain
in the corpus and currently fail the explicit comparison. An approval request
for those exact differences is pending; the test contains no automatic waiver.
