## ADDED Requirements

### Requirement: JDBC Agent Array resources have session ownership

The JDBC Agent SHALL register each created `java.sql.Array` under an opaque session-local handle. Explicit free SHALL invoke the driver once and invalidate the handle only after successful release. A failed release SHALL remain retryable. Closing a logical connection or session SHALL attempt to free all remaining arrays, aggregate failures, and continue closing the connection.

#### Scenario: Explicit release and logical close

- **WHEN** a client frees a registered Array and later closes the logical connection
- **THEN** the Array is released exactly once, cannot be read through its former handle, and unrelated session resources remain isolated

#### Scenario: Failed release

- **WHEN** a driver's `Array#free` fails
- **THEN** the handle remains registered for retry, and closing the session still attempts resource cleanup

### Requirement: JDBC Agent Array methods reflect real driver behavior

The JDBC Agent adapter SHALL expose only Array operations implemented by its protocol. Values, SQL NULL, one-based ranges, metadata, result-set cursors, and release SHALL preserve the driver contract or propagate the driver's error. Unsupported overloads SHALL return the standard SQLState `0A000` before accessing the driver.

#### Scenario: H2 Array roundtrip

- **WHEN** a caller creates an H2 Array with mixed values and SQL NULL, reads its metadata, range, and cursor, then frees it
- **THEN** the returned values and one-based indices match H2 and further reads fail with the standard closed-resource behavior
