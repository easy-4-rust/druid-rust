## ADDED Requirements

### Requirement: JDBC Agent NClob preserves national-character resource identity

The JDBC Agent SHALL create an actual driver `java.sql.NClob` and expose it as the canonical RDBC `NClob`, whose inherited Clob methods retain Java UTF-16 unit and one-based position semantics. A successful ordinary Clob creation SHALL NOT be treated as proof that NClob creation succeeded. Driver SQLState and vendor code SHALL be preserved on NClob failures.

#### Scenario: Driver creates an NClob

- **WHEN** an external RDBC consumer invokes `create_n_clob` against a driver that supports it
- **THEN** the returned resource is an NClob, supports its positively verified inherited operations, and preserves supplementary and isolated UTF-16 units

#### Scenario: Driver rejects NClob creation or an inherited operation

- **WHEN** the real driver rejects NClob creation or a requested operation
- **THEN** the original SQLState and vendor code are visible and no ordinary Clob is substituted

### Requirement: JDBC Agent NClob has session and child-stream ownership

Each NClob SHALL have a session-local opaque handle. Explicit free SHALL close child streams before freeing the driver resource, retain the handle after failed free for retry, and invalidate it after successful free. Logical connection, pooled-owner, and session close SHALL invalidate outstanding NClob and child handles while attempting remaining cleanup and preserving close errors.

#### Scenario: Free with outstanding child stream

- **WHEN** a consumer frees an NClob while a reader or writer remains open
- **THEN** subsequent parent and child operations fail as closed, the driver resource is freed at most once, and any failed free remains retryable

#### Scenario: Pooled owner closes a logical NClob

- **WHEN** a pooled logical connection or its owner closes with an NClob and child stream still open
- **THEN** those handles cannot operate after close and the underlying resource cleanup follows the owner lifecycle

### Requirement: JDBC Agent NClob capability claims follow real-driver evidence

The adapter SHALL advertise only NClob operations proven by positive real-driver tests. Inherited methods not implemented by the adapter SHALL report SQLState `0A000`; driver-specific unsupported results SHALL preserve the driver's original errors. Bounded character and ASCII stream transfers SHALL retain ordering, EOF, cancellation, and parent invalidation without materializing an entire large NClob in one protocol response.

#### Scenario: H2 preserves unsupported search and truncation errors

- **WHEN** an external consumer exercises NClob search and truncation against H2 2.3.232
- **THEN** the adapter preserves the driver's SQLState `HYC00` and vendor code `50100` for each operation, and does not claim either operation as successful H2 behavior

#### Scenario: H2 rejects NClob ASCII writer creation

- **WHEN** an external consumer requests an ASCII output stream from an H2 2.3.232 NClob
- **THEN** the adapter preserves the driver's `HYC00`/`50100` error while the NClob remains releasable

#### Scenario: HSQLDB verifies successful search and truncation

- **WHEN** an external consumer exercises NClob search and truncation against HSQLDB 2.7.4
- **THEN** search returns the verified one-based position and truncation leaves the verified UTF-16 length and contents; the capability mask includes only positively verified adapter operations

#### Scenario: More than one protocol page

- **WHEN** a consumer writes and reads an NClob larger than one protocol page
- **THEN** the returned UTF-16 units are exact, each transfer respects the page bound, and closing the parent invalidates the stream; HSQLDB also verifies successful bounded ASCII transfer
