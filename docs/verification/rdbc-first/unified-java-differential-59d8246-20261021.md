# Unified Java differential verification — 2026-10-21

Source: `59d8246d2d5a91858aecbf7cf950f7103687efe7`. Product Rust source remains the `5bb051f` revision. This batch refreshes the previously separate observations after the Prepared/Callable SQL-context fix.

Command: `cargo +1.97.1 test -p druid-bench --locked --features jdbc-agent --tests --no-fail-fast -- --ignored --nocapture`.

Actual Cargo exit: **101**. All nine opt-in live differential targets executed: four passed and five failed. The frozen Java Druid 1.2.28 checkout, its build classpath and explicit JDK were supplied; the Wall table case used the rebuilt JDBC Agent and the same H2 1.4.199 as the Java oracle.

Across the nine independent scopes: **636 observations = 620 MATCH + 2 APPROVED_STANDARD_DIFFERENCE + 14 MISMATCH**. This aggregate does not replace the original frozen 59-case denominator, whose partition remains 47/2/10. Only the two approved closeOnCompletion/isCloseOnCompletion differences are allowed. The 14 mismatches correspond to the existing [pending decisions](pending-acceptance-decisions.md); none were waived or converted into passing tests.

The [receipt](unified-java-differential-59d8246-20261021.tsv) records every raw comparison path and SHA-256. Full log: `D:/druid-rust-tooling/unified-java-differential-59d8246-20261021.log`, SHA-256 `bbb557ae06046ec3563947e9e9fe6666de49cb804d2460e16f06337adcfc4a72`. No compiled artifacts or archives are included.
