## ADDED Requirements

### Requirement: Release dependency graph excludes known vulnerable ZIP extraction

The release build SHALL resolve a patched ZIP implementation for DuckDB's build dependency and SHALL pass the RustSec vulnerability gate without ignoring `RUSTSEC-2026-0367`, `RUSTSEC-2026-0368`, or `RUSTSEC-2026-0369`.

#### Scenario: Audited release dependency graph

- **WHEN** CI audits the committed lockfile and builds the DuckDB-enabled workspace
- **THEN** `zip 6.0.0` is absent, the three advisories are not waived, and DuckDB behavior tests pass
