# Druid JDBC Agent

This directory builds the Druid-owned JVM release asset used by
`druid-wrapper::jdbc_agent`. It is not a fourth Rust crate and does not contain a
connection pool. One process owns one raw JDBC `Connection`; DruidPool remains the
only pooling authority.

## Build and test

Use JDK 17 or 21. The build creates `target/druid-jdbc-agent.jar`; vendor JDBC
drivers are deliberately not shaded into it.

```bash
mvn verify
```

The contract test uses H2 in Maven test scope and covers framed connect, DDL, bound
update, query, typed rows, and close. The repository workflow defines the complete
Rust-to-Agent H2 contract for JDK 17/21 on Linux, macOS, and Windows; the first remote
matrix result is still pending.

## DAP1 boundary

Each message is a four-byte big-endian length followed by UTF-8 JSON. Both peers
enforce a 16 MiB frame limit. Requests carry `protocolVersion`, `requestId`,
`operation`, and `payload`; responses echo the first two fields and return either a
payload or a structured JDBC exception. Standard output is protocol-only and SLF4J
logs go to standard error.

Supported operations currently include connect, query, update, prepared parameter
binding, generic execute, begin/commit/rollback, liveness, auto-commit, read-only,
transaction isolation, and close. Unimplemented JDBC breadth remains absent from the
capability declaration rather than returning fake success.

## Driver supply

The Agent relies on JDBC ServiceLoader drivers placed on its explicit classpath.
Use the `druid-driver` admin binary to install content-addressed JARs and validate
SHA-256 before constructing that classpath. Commercial drivers must be obtained and
provided by an authorized user; this repository does not redistribute them.
