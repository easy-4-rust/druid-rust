## ADDED Requirements

### Requirement: JDBC Agent Blob operations preserve driver behavior

The JDBC Agent SHALL expose a session-owned `java.sql.Blob` through the canonical RDBC Blob contract. Length, one-based byte ranges and search positions, zero-based source offsets for ranged writes, truncation, and release SHALL preserve the driver's values and SQL errors. A capability bit SHALL be advertised only after its operations have positive adapter evidence; an unsupported operation SHALL return the standard `0A000` error without a false success.

#### Scenario: H2 byte operations and original unsupported errors

- **WHEN** a caller creates an H2 Blob, writes bytes with both setter overloads, reads a one-based range, then requests search and truncation
- **THEN** byte values and the zero-based source offset are exact, while H2's unsupported search and truncation retain the original `HYC00` SQLState and `50100` vendor code

#### Scenario: HSQLDB successful search and truncation

- **WHEN** a caller writes an HSQLDB Blob, searches a two-byte pattern from a one-based start, and truncates it
- **THEN** search returns the one-based match position or no match, and truncation changes the length and remaining bytes exactly

#### Scenario: Driver rejection and unsupported operation

- **WHEN** the driver rejects a position or a method is not implemented by the adapter
- **THEN** the original driver SQLState/vendor code is retained, or the unsupported method returns `0A000`, respectively

### Requirement: JDBC Agent Blob streams and release have bounded ownership

Blob input and output streams SHALL retain their own cursor and close state under the parent Blob and session. Transfers SHALL be bounded per protocol request, not eagerly materialize an entire large Blob. Explicit release SHALL close child streams before freeing the driver Blob; a failed driver `free` SHALL leave the Blob handle retryable. Logical connection, pooled owner, and session close SHALL invalidate all child handles and attempt remaining cleanup while preserving failures.

#### Scenario: Large streamed Blob

- **WHEN** a caller reads and writes a Blob larger than one protocol page through the standard streams
- **THEN** bytes remain ordered and exact, each transfer respects the configured page bound, and EOF and explicit close are observable

#### Scenario: Release with outstanding streams

- **WHEN** a Blob is freed or its owner closes while a child stream remains open
- **THEN** later stream and Blob operations fail as closed, underlying resources are released at most once, and a failed Blob release remains retryable
