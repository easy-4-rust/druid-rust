## ADDED Requirements

### Requirement: Task 6 migration evidence is current and exact

The repository SHALL bind every retired standard identity to one authoritative disposition and exact target, and SHALL keep generated migration documentation synchronized with that authority. Preserved behavior SHALL have source-bound method and test parity evidence. Task 6 SHALL remain open while any required API, rustdoc, SemVer, workspace, feature, or MSRV gate lacks an actual result.

#### Scenario: Migration authority and generated views agree

- **WHEN** the Task 6 migration authority verifier runs against the current checkout
- **THEN** authority rows, the physical-standardization manifest, and the generated migration table agree field-for-field
- **AND** every non-deleted target is bound to the exact canonical standard surface

#### Scenario: Preserved method and test behavior is certified

- **WHEN** the Task 6 method and deleted-test parity gates run
- **THEN** every required baseline method and test identity has a unique current target or an explicitly approved identity-only disposition
- **AND** behavioral targets execute successfully with evidence bound to current source hashes

#### Scenario: Public and build compatibility is certified

- **WHEN** Task 6 acceptance is evaluated
- **THEN** the current public API matches its reviewed snapshot, rustdoc and SemVer checks have actual results, and required workspace, feature, and MSRV checks have actual results
- **AND** any missing tool, unrun command, stale hash, or failed required gate keeps Task 6 open
