# JDBC Agent callable statement execution

## Why

`druid-rdbc` declares the JDBC 4.2 `CallableStatement` contract and Druid has a logical pooled proxy, but `JdbcAgentConnection` advertises `callable_statements: false` because the Agent protocol has no `prepare_call` operation. A real JDBC driver cannot currently supply an owned callable handle to the standard path. Declaration and logical-proxy tests do not prove physical adapter capability.

## What Changes

Add session-owned callable preparation, execution, OUT registration and reading to the Java Agent protocol and a Rust `JdbcAgentConnection::prepare_call` implementation. Preserve the standard `Connection -> CallableStatement` owner boundary, indexed and named parameter identity, SQL NULL, JDBC error details, cancellation, result and close lifetimes. Advertise callable capability only when the connected driver and implemented protocol path actually support the requested operations. Prove supported behavior against H2 and HSQLDB, and verify unsupported methods return precise standard errors without false capability claims.

## Scope

This change implements the JDBC Agent physical adapter and its protocol; it does not imply that SQLx, Toasty, HTTP SQL, libSQL, or DuckDB implement stored procedures. It does not close all 69 JDBC object rows or the P04 Task 8 release gate by itself. `rdbc-jdbc-method-conformance` remains the canonical specification family, with this change adding only the JDBC Agent callable delta.

## Impact

Java Agent session/protocol and tests, Rust JDBC Agent transport and callable adapter, standard capability evidence, real-database matrix, public API and SemVer review. No new repository, branch, CLI, or runtime dependency is required.
