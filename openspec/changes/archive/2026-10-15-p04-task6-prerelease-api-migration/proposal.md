# Proposal

## Why

The current `0.0.0-design` API is still pre-release, and the JDBC alignment removes duplicate pooled-object and filter-chain surfaces in favor of one standard contract. The SemVer gate already records 2,578 inherent-method and 390 trait-method findings, but its category-level allowlist does not preserve each affected source identity for review.

## What Changes

- **BREAKING** Record every removed inherent or trait method identity from the SemVer baseline, including repeated public-path occurrences, and bind the record to the existing approved SemVer categories.
- Record each unique old identity, its baseline source location, number of SemVer occurrences, migration route, and the canonical standard contract entry or retired internal surface.
- Keep `DataSource` as the supported entry, followed by `Connection`, `Statement`, and the other JDBC standard contracts; do not restore duplicate transition methods.
- Add a fail-closed check so a changed SemVer report cannot silently drift from the item-level migration ledger.

## Capabilities

### New Capabilities

None. This change records and verifies an already-approved pre-release API migration; it does not change runtime behavior.

### Modified Capabilities

None.

## Impact

Affected files are the Task 6 SemVer verifier and its tests, the SemVer allowlist and migration ledger under `api/`, and Task 6 verification evidence. No runtime crate behavior or dependency changes are included.
