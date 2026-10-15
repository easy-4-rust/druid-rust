## ADDED Requirements

### Requirement: JDBC Agent close cascades preserve every JDBC failure

The JDBC Agent SHALL attempt to close every owned ResultSet, cursor, and PreparedStatement in a close cascade. If multiple JDBC closes fail, the first `SQLException` SHALL remain primary and every later failure SHALL be attached as a suppressed exception. The corresponding cursor and prepared-statement registrations SHALL be removed even when closing fails.

#### Scenario: Cursor and its temporary statement both fail to close

- **WHEN** closing a cursor produces a ResultSet close failure followed by a temporary PreparedStatement close failure
- **THEN** the ResultSet failure is primary and the statement failure is suppressed
- **AND** both close operations were attempted

#### Scenario: Multiple cursors and their prepared statement fail to close

- **WHEN** closing a registered PreparedStatement produces failures from two related cursors and the PreparedStatement
- **THEN** a cursor failure is primary and the other two failures remain observable as suppressed exceptions
- **AND** all three closes were attempted and all session registrations were removed
