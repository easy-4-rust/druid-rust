## ADDED Requirements

### Requirement: Bounded child parity must report its own outcome

The final no-physical-standard gate SHALL await Task 6 method parity without a shorter parent deadline. The method-parity child SHALL retain finite deadlines on its subprocesses, and any child failure SHALL fail the final gate.

#### Scenario: Cold integration run exceeds the old parent deadline

- **WHEN** Task 6 method parity continues past 1,800 seconds while its bounded subprocesses are still running
- **THEN** the parent waits for the child's exit status instead of reporting an outer timeout

#### Scenario: Child reports failure

- **WHEN** Task 6 method parity exits nonzero
- **THEN** the final gate reports failure with the child's diagnostic output
