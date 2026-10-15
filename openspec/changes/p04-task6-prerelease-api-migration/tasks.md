# Tasks

## 1. Item-level SemVer migration evidence

- [x] 1.1 Parse the `inherent_method_missing` and `trait_method_missing` sections and add tests for duplicate aggregation, ANSI output, source normalization, and malformed findings; verify the focused Python test passes.
- [x] 1.2 Generate the item-level TSV from the completed CI reports and verify exact counts of 2,578 inherent findings, 390 trait findings, and 2,968 total occurrences.
- [x] 1.3 Classify each unique identity by canonical standard contract family or explicitly retired internal surface; verify all 831 unique identities have a route and no identity is omitted.

## 2. SemVer gate and review evidence

- [x] 2.1 Extend the existing SemVer verifier to compare live finding identities and occurrence counts with the migration TSV; verify passing and drifted-fixture tests.
- [x] 2.2 Update the two approved category reasons to reference the item ledger and document the pre-release `DataSource` migration route; verify the SemVer gate and strict OpenSpec validation.
- [x] 2.3 Run the Task 6 focused SemVer verifier, review `git diff --check`, and record CI run, baseline, report hashes, counts, and limitations in the evidence note.
