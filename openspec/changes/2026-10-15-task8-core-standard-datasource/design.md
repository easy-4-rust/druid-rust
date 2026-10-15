# Design

## Context

The canonical `DataSource` trait extends `CommonDataSource`, `Wrapper`, and `Sync`. `DruidDataSource` already implements `CommonDataSource`, owns `DruidPool`, and returns `DruidPooledConnection`, which implements the canonical `Connection` trait. The wrapper's `DruidRdbcDataSource` is a separate credential-aware adapter around this same pool.

## Goals / Non-Goals

**Goals:** Let core-pool consumers use the same standard trait object and preserve native lease ownership and error behavior.

**Non-Goals:** Add a second pool, infer database credentials from the core object, or promote the entire JDBC semantic inventory to LIVE.

## Decisions

Implement `Wrapper` and canonical `DataSource` directly on `DruidDataSource`. The wrapper view returns `self` for `Wrapper` and `DataSource` target IDs; no concrete downcast or new object is introduced. The standard `connection` method delegates to the existing native `get_connection`, boxes the returned pooled connection, and converts its existing error. Keep the trait's default `connection_with_credentials` refusal because the core pool cannot authenticate a caller-supplied pair; the wrapper retains its explicit credential-aware behavior.

An external integration test compiles against `dyn druid_rdbc::data_source::DataSource`, opens and closes a connection, checks the same pool's counts, and verifies unwrap identity and the `0A000` refusal. Existing product-facade tests remain unchanged.

## Risks / Trade-offs

- [Added public trait implementations could change method-call inference] → Use qualified calls in tests and keep existing inherent and product-facade methods intact; inspect public API and SemVer CI.
- [A boxed logical connection could obscure return-to-pool behavior] → Verify active/idle counts before and after standard close in an external consumer test.

## Migration Plan

This is an additive pre-release implementation. Consumers may opt into the canonical trait without changing existing `druid::sql::DataSource` calls. Revert only this implementation and its test if an unexpected compatibility regression is found.
