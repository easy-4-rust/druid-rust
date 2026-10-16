# rdbc-standard-purity Specification

## Purpose
Define how the final RDBC purity gate enumerates reachable standard Rust modules and rejects hidden or ambiguous source before enforcing product-independent contracts.

## Requirements

### Requirement: Canonical file-module roots remain in the purity denominator

The final RDBC standard-purity gate SHALL accept a public canonical root resolved by Rust to either `module.rs` or `module/mod.rs`. It SHALL inspect that root and every physical Rust child beneath `module/` for forbidden product/runtime identities and path provenance. It SHALL reject ambiguous roots containing both root forms and incomplete directory roots.

#### Scenario: Reachable file module
- **WHEN** `lib.rs` declares `pub mod url;` and `url.rs` defines the canonical URL object
- **THEN** the purity gate includes `url.rs` and passes if all source checks pass

#### Scenario: Hidden pollution or ambiguous root
- **WHEN** `url.rs` or an undeclared child under `url/` contains forbidden content, or `url/mod.rs` also exists
- **THEN** the gate fails rather than excluding the source or choosing one root arbitrarily

### Requirement: Final purity proves standard prelude macro provenance

The final RDBC standard-purity gate SHALL accept bare `assert!`, `assert_eq!`, and `vec!` invocations in reachable canonical source only when no local textual macro, imported binding, or crate-root export shadows the standard Rust prelude identity. The gate SHALL continue to reject unknown, raw, and ambiguously bound macro invocations.

#### Scenario: Unshadowed standard prelude macro

- **WHEN** reachable canonical source invokes an unshadowed `assert!`, `assert_eq!`, or `vec!`
- **THEN** the final purity gate accepts that invocation and continues checking the rest of the source

#### Scenario: Shadowed standard macro name

- **WHEN** a local macro definition or imported binding shadows one of those names
- **THEN** the final purity gate rejects the invocation with a provenance error
