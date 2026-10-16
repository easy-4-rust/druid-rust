## ADDED Requirements

### Requirement: Static JDBC declarations have explicit capability disposition

The Task 8 semantic verifier SHALL allow `NOT_APPLICABLE` runtime capability state only for static DTO, enum-kind, or module declarations with no adapter or logical implementation, excluding the active DriverManager registry. It SHALL reject the same disposition on behavioral contracts or implemented rows. This evidence-only classification SHALL leave the frozen Java source mapping unchanged and SHALL NOT satisfy pending adapter or logical behavior requirements.

#### Scenario: Static value declaration

- **WHEN** a DTO, enum-kind, or module has neither adapter nor logical implementation
- **THEN** the inventory may mark its runtime capability as `NOT_APPLICABLE`, retaining `V0_STATIC` evidence and the original source digest

#### Scenario: Behavioral declaration

- **WHEN** a trait, callback, active registry, or implemented row claims static `NOT_APPLICABLE` capability
- **THEN** verification fails without modifying the Java source oracle
