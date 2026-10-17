## ADDED Requirements

### Requirement: Repeated Admin session churn remains within a measured memory budget

The Admin repository release gate SHALL run at least 50 TTL-separated generations of service sessions with 32 identities and two datasource projections per generation. It SHALL measure process physical memory during the workload on Linux, macOS and Windows, reject unavailable measurements, and fail if any observed sample exceeds 128 MiB. It SHALL verify that expired identities are removed, only the current generation remains, and capacity rejection does not mask a memory leak. The report SHALL label Unix RSS or Windows working set and state that this is sampled memory, not an operating-system lifetime peak.

#### Scenario: Expired sessions are replaced repeatedly

- **WHEN** 50 generations of 64 projections are admitted after the preceding generation's TTL expires
- **THEN** the repository retains 64 current projections, evicts 3,136 expired projections, and records zero capacity rejections
- **AND** prior boot identities are not visible

#### Scenario: Process memory is sampled across platforms

- **WHEN** the churn test runs on Linux, macOS or Windows
- **THEN** it reports the platform memory metric and sampled high-water mark and fails above 128 MiB
- **AND** an unavailable process-memory reading fails the test
