## ADDED Requirements

### Requirement: HTTP SQL and libSQL do not advertise unsupported standard connection capabilities

The HTTP SQL and libSQL adapters SHALL advertise no transaction, savepoint, catalog, schema, or callable capability until those operations have backend evidence. Calls to transaction, savepoint, and callable operations SHALL return the standard unsupported-operation error. SQL Array creation SHALL return the standard feature-not-supported SQLState `0A000`.

#### Scenario: Deterministic transport keeps unsupported capabilities explicit

- **GIVEN** a standard connection created by the HTTP SQL or libSQL factory with its deterministic transport
- **WHEN** the caller reads capabilities and invokes unsupported transaction, savepoint, callable, and SQL Array operations
- **THEN** all five flags are false and each operation returns its documented standard error
- **AND** this test certifies adapter behavior only, not live remote service behavior
