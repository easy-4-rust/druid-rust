## ADDED Requirements

### Requirement: Canonical file-module roots remain in the purity denominator

The final RDBC standard-purity gate SHALL accept a public canonical root resolved by Rust to either `module.rs` or `module/mod.rs`. It SHALL inspect that root and every physical Rust child beneath `module/` for forbidden product/runtime identities and path provenance. It SHALL reject ambiguous roots containing both root forms and incomplete directory roots.

#### Scenario: Reachable file module
- **WHEN** `lib.rs` declares `pub mod url;` and `url.rs` defines the canonical URL object
- **THEN** the purity gate includes `url.rs` and passes if all source checks pass

#### Scenario: Hidden pollution or ambiguous root
- **WHEN** `url.rs` or an undeclared child under `url/` contains forbidden content, or `url/mod.rs` also exists
- **THEN** the gate fails rather than excluding the source or choosing one root arbitrarily
