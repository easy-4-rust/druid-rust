## ADDED Requirements

### Requirement: JDBC Agent Clob resources have retryable session ownership

The JDBC Agent SHALL register each created `java.sql.Clob` under a session-local opaque handle. Explicit free SHALL close child streams before invoking the driver and SHALL invalidate the handle only after a successful driver free. A failed driver free SHALL retain the handle for retry. Logical connection and session close SHALL attempt all remaining cleanup, aggregate failures, and continue closing the JDBC connection.

#### Scenario: Failed release and later close

- **WHEN** the driver rejects the first `Clob.free()` and later accepts a retry
- **THEN** the original handle remains usable for retry, successful free invalidates it, and the driver is not freed twice

### Requirement: JDBC Agent Clob operations preserve character semantics and driver errors

The standard adapter SHALL preserve Java UTF-16 unit length, one-based Clob positions, zero-based source offsets, SQL NULL boundaries, and driver SQLState/vendor codes for supported operations. It SHALL report only positively verified capabilities; unsupported operations SHALL return SQLState `0A000` without a false success.

#### Scenario: Character operations against real JDBC drivers

- **WHEN** an external RDBC consumer writes and reads a Clob containing a supplementary character and tests range, search, and truncation against H2 and HSQLDB
- **THEN** UTF-16 units and successful driver results are exact, while unsupported driver operations retain their original SQL errors

### Requirement: JDBC Agent Clob streams use bounded transfers and parent ownership

Character and ASCII streams SHALL transfer bounded pages, preserve order and EOF, and become invalid after explicit free, logical close, pooled-owner close, or session close. Cancellation SHALL not leak a child handle or replay accepted data.

#### Scenario: Large streamed Clob

- **WHEN** a consumer reads and writes more than one protocol page and then closes the parent with a child still open
- **THEN** data remains exact within the page bound and later child operations fail as closed
