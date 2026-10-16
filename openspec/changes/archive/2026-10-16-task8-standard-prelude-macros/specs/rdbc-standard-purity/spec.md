## ADDED Requirements

### Requirement: Final purity proves standard prelude macro provenance

The final RDBC standard-purity gate SHALL accept bare `assert!`, `assert_eq!`, and `vec!` invocations in reachable canonical source only when no local textual macro, imported binding, or crate-root export shadows the standard Rust prelude identity. The gate SHALL continue to reject unknown, raw, and ambiguously bound macro invocations.

#### Scenario: Unshadowed standard prelude macro

- **WHEN** reachable canonical source invokes an unshadowed `assert!`, `assert_eq!`, or `vec!`
- **THEN** the final purity gate accepts that invocation and continues checking the rest of the source

#### Scenario: Shadowed standard macro name

- **WHEN** a local macro definition or imported binding shadows one of those names
- **THEN** the final purity gate rejects the invocation with a provenance error
