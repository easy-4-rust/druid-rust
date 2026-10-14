# P04 Task 7 SQLite adapter evidence

## Why

Existing Task 7 evidence proves resource lifecycle contracts with controlled drivers and SQLx/Toasty result-set behavior, but it explicitly leaves real database resource behavior `NOT_RUN`. The SQLx SQLite adapter uses default resource factories, so the missing evidence is whether a live SQLite connection reports its supported connection capabilities and returns the exact unsupported error for JDBC resource factories while still executing its asynchronous result set.

## Scope

- Add one real SQLx SQLite in-memory test at the adapter boundary.
- Assert the reported connection capability descriptor and exact `0A000` unsupported results for Array, Blob, Clob, NClob, SQLXML, and Struct factories.
- Execute a multi-row query through the standard async ResultSet interface and verify values, EOF, and explicit close.
- Record that this proves SQLx SQLite behavior only; it does not certify other backends or claim those resources are supported.

## Out of scope

- Implementing database resource factories for SQLite or other backends.
- Changing public API or SemVer.
- Claiming JDBC TCK or all-backend compatibility.

## Acceptance

The targeted SQLx SQLite test passes using an actual in-memory SQLite connection and OpenSpec validation passes. All resource operations unsupported by this adapter remain explicitly unsupported with SQLState `0A000`.
