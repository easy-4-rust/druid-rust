## ADDED Requirements

### Requirement: Cross-platform repository memory budget evidence

The Admin repository capacity test MUST measure process physical memory during the bounded repository fill on Linux, macOS, and Windows. The test MUST reject unavailable measurements rather than silently pass, MUST preserve the existing 128 MiB limit, and MUST identify the metric as sampled RSS on Unix or working set on Windows. The evidence MUST state that in-process sampling is a sampled high-water mark and not an operating-system lifetime peak.

#### Scenario: Repository fills close to its serialized-state limit

- **WHEN** the test inserts datasource projections until the 64 MiB retained-state budget is nearly full
- **THEN** it samples process physical memory during the fill and fails if any observed sample exceeds 128 MiB
- **AND** it reports accepted rows, retained bytes, sampled physical-memory high-water bytes, and the platform metric name

#### Scenario: The platform cannot provide a measurement

- **WHEN** the process-memory probe returns no reading
- **THEN** the test fails with an explicit measurement error instead of omitting the budget assertion

#### Scenario: CI runs on supported operating systems

- **WHEN** the repository-capacity test executes on Linux, macOS, or Windows
- **THEN** the same workload and ceiling run on each platform
- **AND** the recorded result labels RSS for Unix and working set for Windows
