## ADDED Requirements

### Requirement: SQLx fallback capability claims require backend evidence

The SQLx adapter SHALL advertise transactions and savepoints only for its positively verified native SQLite, MySQL, and PostgreSQL backends. The generic `Any` fallback SHALL advertise neither capability until a dedicated real-driver contract proves support. The capability descriptor SHALL remain consistent after the physical connection moves into its owned actor; conservative metadata SHALL NOT replace driver errors from attempted operations.

#### Scenario: Generic fallback before and after actor handoff

- **WHEN** a SQLx `Any` connection is queried before and after creating an owned statement
- **THEN** both capability snapshots report transactions and savepoints as false

#### Scenario: Verified native backend

- **WHEN** a native SQLite, MySQL, or PostgreSQL adapter reports its capabilities
- **THEN** transactions and savepoints remain true as established by its real-database contract
