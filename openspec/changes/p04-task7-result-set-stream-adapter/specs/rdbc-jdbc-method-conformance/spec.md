## ADDED Requirements

### Requirement: SQLx asynchronous ResultSet preserves an in-flight row when next is cancelled
The SQLx asynchronous ResultSet SHALL retain a response that was accepted for an in-flight `next` call when the caller cancels that future. A subsequent `next` call on the same open ResultSet SHALL observe that response exactly once.

#### Scenario: Retry next after cancellation
- **WHEN** `next` is pending on an accepted row response and the caller cancels that `next` future
- **AND** the response later supplies one row
- **THEN** a subsequent `next` returns `true` for that row
- **AND** the row number advances once

### Requirement: SQLx SQLite resource and asynchronous ResultSet evidence stays backend-scoped
Evidence for capability reporting, unsupported resource factories, and live asynchronous ResultSet iteration SHALL identify SQLx SQLite as its tested backend and SHALL NOT imply equivalent behavior for other adapters.

#### Scenario: Report unsupported SQLx SQLite resources truthfully
- **WHEN** the live SQLx SQLite test invokes Array, Blob, Clob, NClob, SQLXML, or Struct creation
- **THEN** each unsupported factory returns SQLState `0A000`
- **AND** the test records that this result applies only to SQLx SQLite

#### Scenario: Iterate and close a live SQLx SQLite result set
- **WHEN** a standard Statement executes a multi-row query against in-memory SQLx SQLite
- **THEN** rows are returned in order, EOF is observed, and explicit ResultSet close succeeds
