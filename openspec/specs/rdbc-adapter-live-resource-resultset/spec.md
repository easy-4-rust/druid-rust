# rdbc-adapter-live-resource-resultset Specification

## Purpose
TBD - created by archiving change p04-task7-cross-backend-resource-resultset-evidence. Update Purpose after archive.

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
