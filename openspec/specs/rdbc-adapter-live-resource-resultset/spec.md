# rdbc-adapter-live-resource-resultset Specification

## Purpose
Define live SQLx and Toasty adapter evidence for truthful resource capability errors and the standard asynchronous ResultSet lifecycle across SQLite, PostgreSQL, and MySQL.

## Requirements

### Requirement: Live adapters reject unimplemented resource factories precisely

The SQLx and Toasty adapters SHALL return feature-not-supported with SQLState `0A000` for Array, Blob, Clob, NClob, SQLXML, and Struct creation when those factories are not implemented. Live tests SHALL execute the same assertions against SQLite, PostgreSQL, and MySQL in the configured driver matrix. This requirement does not assert support for the resource types.

#### Scenario: Resource factory on a live adapter
- **WHEN** a caller invokes any of the six resource factories through a SQLx or Toasty standard connection
- **THEN** the factory returns `SqlException` with SQLState `0A000`, without producing a resource

### Requirement: Live adapters expose an asynchronous ResultSet lifecycle

The SQLx and Toasty adapters SHALL expose committed query rows through the standard asynchronous Statement/ResultSet contract. Live tests SHALL verify row value, EOF, explicit ResultSet close, and Statement close on each configured backend.

#### Scenario: Read the committed row
- **WHEN** the caller executes a SELECT after a transaction and savepoint rollback
- **THEN** asynchronous `next` returns the committed row, subsequently returns false at EOF, and explicit close succeeds

### Requirement: SQLx fallback capability claims require backend evidence

The SQLx adapter SHALL advertise transactions and savepoints only for its positively verified native SQLite, MySQL, and PostgreSQL backends. The generic `Any` fallback SHALL advertise neither capability until a dedicated real-driver contract proves support. The capability descriptor SHALL remain consistent after the physical connection moves into its owned actor; conservative metadata SHALL NOT replace driver errors from attempted operations.

#### Scenario: Generic fallback before and after actor handoff

- **WHEN** a SQLx `Any` connection is queried before and after creating an owned statement
- **THEN** both capability snapshots report transactions and savepoints as false

#### Scenario: Verified native backend

- **WHEN** a native SQLite, MySQL, or PostgreSQL adapter reports its capabilities
- **THEN** transactions and savepoints remain true as established by its real-database contract
