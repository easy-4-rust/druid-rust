# admin-runtime Specification

## Purpose
Define measurable operating limits for the Admin service so repository growth and process memory remain observable and bounded on every supported CI operating system.

## Requirements

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

### Requirement: Authenticated Admin JSON redacts sensitive field aliases

The secure Admin JSON response policy SHALL redact database URL and connection-string aliases, username aliases, SQL text, parameter values, and driver error-message aliases after normalizing field-name case and separators. It SHALL recurse through non-sensitive nested objects and arrays while preserving safe counters. Compatibility mode SHALL retain its existing unchanged-response behavior.

#### Scenario: Nested aliases in an authenticated management response

- **WHEN** a management JSON response contains `databaseUrl`, `connectionString`, `dbUserName`, `statementSql`, `bindParameters`, or `exceptionMessage` in nested objects or arrays
- **THEN** each sensitive value is replaced by `[REDACTED]`
- **AND** safe numeric counts remain visible

#### Scenario: Local compatibility response

- **WHEN** compatibility redaction mode handles the same JSON
- **THEN** its values remain unchanged

### Requirement: Repeated Admin session churn remains within a measured memory budget

The Admin repository release gate SHALL run at least 50 TTL-separated generations of service sessions with 32 identities and two datasource projections per generation, each carrying at least 60 KiB of SQL projection text. It SHALL measure process physical memory during the workload on Linux, macOS and Windows, reject unavailable measurements, and fail if any observed sample exceeds 128 MiB. It SHALL verify that expired identities are removed, only the current generation remains, and capacity rejection does not mask a memory leak. The report SHALL label Unix RSS or Windows working set and state that this is sampled memory, not an operating-system lifetime peak.

#### Scenario: Expired sessions are replaced repeatedly

- **WHEN** 50 generations of 64 projections are admitted after the preceding generation's TTL expires
- **THEN** the repository retains 64 current projections, evicts 3,136 expired projections, and records zero capacity rejections
- **AND** prior boot identities are not visible

#### Scenario: Process memory is sampled across platforms

- **WHEN** the churn test runs on Linux, macOS or Windows
- **THEN** it reports the platform memory metric and sampled high-water mark and fails above 128 MiB
- **AND** an unavailable process-memory reading fails the test

### Requirement: Admin network faults have repeatable process-level release evidence

The scheduled/manual release workflow SHALL repeatedly execute real HTTP/gRPC tests for duplicate batches, sequence gaps, Admin restart with pending work, unavailable Admin shutdown, invalid remote authentication, and SIGTERM while a command ACK is pending. It SHALL require each behavioral test to pass, capture raw output, record each scenario's process-level completion p50/p95/p99 and Linux child peak RSS, and retain a machine-readable result on failure. Each run SHALL also report the asserted sequence, projection, rejection, unflushed work, or pending-release count relevant to that scenario; shutdown scenarios SHALL report their measured shutdown duration. Missing or contradictory observations SHALL fail the workflow. The report SHALL label process times as whole-scenario times including test setup, not request-only latency. Numeric limits SHALL be accepted after baseline measurement and enforced before P06 Task 5 closes.

#### Scenario: The complete fault matrix succeeds

- **WHEN** every named fault scenario is executed repeatedly against real transports
- **THEN** each run preserves its behavior assertions and records duration and process memory without silently omitting a scenario

#### Scenario: A scenario or measurement fails

- **WHEN** an executable is missing, a test fails or times out, output is malformed, or an accepted limit is exceeded
- **THEN** the release workflow fails and retains available raw logs and JSON failure evidence
