## ADDED Requirements

### Requirement: SQLx SQLite reports resource support truthfully
The SQLx SQLite adapter SHALL report its connection-level capabilities and return the standard feature-not-supported error for resource factories it does not implement. An adapter-level test against a real in-memory SQLite connection SHALL verify the capability descriptor and the exact SQLState `0A000` for Array, Blob, Clob, NClob, SQLXML, and Struct creation. This evidence applies only to SQLx SQLite and MUST NOT imply that another backend supports or rejects the same operation.

#### Scenario: Unsupported resource creation on SQLx SQLite
- **WHEN** a caller invokes a resource factory on a live SQLx SQLite connection for Array, Blob, Clob, NClob, SQLXML, or Struct
- **THEN** the adapter returns a feature-not-supported error with SQLState `0A000`
- **AND** the test does not interpret the resource type's presence in the standard API as backend support

### Requirement: SQLx SQLite asynchronous result sets are verified against a live driver
The SQLx SQLite adapter SHALL expose query rows through the standard asynchronous ResultSet contract. A live in-memory SQLite test SHALL verify ordered row values, end-of-stream behavior, and explicit result-set close.

#### Scenario: Iterate a live SQLx SQLite result set
- **WHEN** a caller executes a multi-row query through the standard Statement and ResultSet interfaces
- **THEN** each row is observable in order through asynchronous `next`
- **AND** `next` returns false after the last row
- **AND** explicit close completes successfully
