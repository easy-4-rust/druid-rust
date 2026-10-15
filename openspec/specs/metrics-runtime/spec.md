# metrics-runtime Specification

## Purpose
Define measurable shutdown and work-drain guarantees for the Metrics runtime under saturated sampling, so release checks can detect stranded work and unbounded shutdown cost.

## Requirements

### Requirement: Saturated Metrics shutdown has numeric release evidence

The runtime benchmark workflow SHALL repeatedly execute the real saturated Metrics shutdown test and save a machine-readable record of shutdown p50/p95/p99, coalesced count, remaining task/work counts, and process peak RSS. It SHALL fail if a run fails, output is missing, coalescing is absent, owned work remains, or an accepted numeric budget is exceeded. It SHALL upload available evidence even when the measurement fails.

#### Scenario: Queue saturates and runtime drains

- **WHEN** the capacity-one runtime samples twenty active datasources and the test is repeated
- **THEN** every run reports at least one coalesced snapshot and zero active tasks and unflushed work after shutdown
- **AND** the measurement records shutdown percentiles and peak RSS and checks their configured limits

#### Scenario: Evidence is missing or exceeds a limit

- **WHEN** any execution fails, its summary is malformed, or a metric exceeds its limit
- **THEN** the measurement exits unsuccessfully and preserves the raw log and available JSON evidence
