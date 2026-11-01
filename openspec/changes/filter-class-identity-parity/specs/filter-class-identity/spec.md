## ADDED Requirements

### Requirement: Preserve exact Filter class identity
FilterManager SHALL compare registered class names case-sensitively, matching Java FilterManager.existsFilter at the frozen source revision.

#### Scenario: Two case-distinct registered classes
- **WHEN** an alias expands to two registered class names differing only in case
- **THEN** both factories execute once and both filters remain in expansion order

#### Scenario: Repeated exact class
- **WHEN** the same class is loaded again directly or through an alias
- **THEN** no additional instance is constructed

#### Scenario: Long direct class names
- **WHEN** distinct class names of at least 128 UTF-16 units differ only in case
- **THEN** direct loading preserves both identities and exact repeat loading is idempotent
