## ADDED Requirements

### Requirement: Authenticated Admin JSON redacts sensitive field aliases

The secure Admin JSON response policy SHALL redact database URL and connection-string aliases, username aliases, SQL text, parameter values, and driver error-message aliases after normalizing field-name case and separators. It SHALL recurse through non-sensitive nested objects and arrays while preserving safe counters. Compatibility mode SHALL retain its existing unchanged-response behavior.

#### Scenario: Nested aliases in an authenticated management response

- **WHEN** a management JSON response contains `databaseUrl`, `connectionString`, `dbUserName`, `statementSql`, `bindParameters`, or `exceptionMessage` in nested objects or arrays
- **THEN** each sensitive value is replaced by `[REDACTED]`
- **AND** safe numeric counts remain visible

#### Scenario: Local compatibility response

- **WHEN** compatibility redaction mode handles the same JSON
- **THEN** its values remain unchanged
