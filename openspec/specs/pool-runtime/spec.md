# pool-runtime Specification

## Purpose
Define observable Pool slow-close, bounded-queue and shutdown behavior under saturation, together with repeatable release measurements and fail-closed numeric budgets.

## Requirements

### Requirement: Slow physical close has reproducible fault-budget evidence

The release workflow SHALL exercise a real DruidPool with blocked physical connection close and a finite close queue. It SHALL record queue capacity and observed peak, rejection and timeout counts, physical close completion, shutdown duration percentiles, and process peak RSS. A run without queue saturation, a run that exceeds queue capacity, a missing measurement, or retained close work SHALL fail. The workflow SHALL preserve raw logs and a machine-readable record on failure. Numeric latency and RSS limits SHALL be based on an observed baseline and enforced before P06 Task 5 can close.

#### Scenario: Close queue saturates and drains

- **WHEN** more connections are discarded than the worker plus configured queue can hold while physical close is blocked
- **THEN** at least one close is rejected, queue depth never exceeds capacity, and shutdown drains or explicitly times out owned work within its configured deadline

#### Scenario: Measurement cannot prove the fault budget

- **WHEN** the test output is missing, malformed, unsaturated, over capacity, or above an accepted numeric limit
- **THEN** the release measurement fails and retains its available log and JSON evidence
