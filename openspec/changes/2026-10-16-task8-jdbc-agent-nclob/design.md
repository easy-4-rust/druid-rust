# Design

## Context

The standard `NClob` trait extends `Clob`, and `Connection#create_n_clob` is already public but defaults to unsupported. The Agent has session-owned Clob handles, bounded UTF-16/ASCII operations, and pooled-owner invalidation. The NClob path must begin with `java.sql.Connection#createNClob`, not `createClob`.

## Goals / Non-Goals

**Goals:** preserve actual NClob identity across Java and Rust; reuse inherited Clob behavior only where its semantics and ownership are identical; bind claims to H2/HSQLDB driver evidence.

**Non-Goals:** change the public RDBC NClob signature, claim all JDBC drivers support NClob, or promote other pending Task 8 rows.

## Decisions

- The Agent invokes `Connection#createNClob` and registers the resulting NClob in its session. The existing Clob operation handlers may accept that handle because JDBC `NClob extends Clob`, but creation and test evidence remain independently identified. This avoids a second implementation of inherited operations while preventing an ordinary Clob fallback.
- Rust uses a dedicated `JdbcAgentNClob` type that implements `NClob` and delegates inherited `Clob` behavior to the existing bounded adapter. The distinct type makes the standard return type and future NClob-specific behavior explicit.
- Existing session ownership and stream close paths remain the authority for retry, cancellation, and pooled close. Tests first prove NClob handles reach those paths; any difference found in driver behavior is handled explicitly rather than assumed from Clob tests.
- Capabilities begin conservative and advance only with positive H2/HSQLDB evidence. Error assertions compare with direct driver outcomes where possible, so an unsupported driver operation is not mistaken for a protocol failure.
- H2 2.3.232 returns `HYC00`/`50100` for NClob search and truncation, whereas HSQLDB 2.7.4 succeeds. The two drivers therefore validate complementary outcomes: H2 proves error fidelity, and HSQLDB proves successful inherited behavior.
- H2 2.3.232 also rejects NClob ASCII output creation with `HYC00`/`50100`. HSQLDB 2.7.4 supplies the positive multi-page ASCII transfer evidence; both drivers supply multi-page UTF-16 and owner-close evidence.

## Risks / Trade-offs

- A driver may reject `createNClob` even when `createClob` works. The external contract records that exact driver result and never substitutes a Clob.
- Shared Clob handlers could conceal a resource-kind mix-up. Tests must reject using a Clob handle as NClob and verify that the Java creation call is `createNClob`.
- The existing stream bridge may have NClob-specific encoding or close differences. More-than-one-page tests and pooled-owner tests cover the shared path before capability promotion.

## Migration Plan

Add protocol/session tests, then Rust external-consumer tests, then implementation and inventory evidence. Reconcile public API and SemVer reports, run the cross-platform matrix, sync the delta spec, and archive only after all gates pass. A rollback removes only the new NClob route and evidence, leaving the Clob route intact.
