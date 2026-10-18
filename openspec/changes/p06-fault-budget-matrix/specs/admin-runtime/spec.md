## ADDED Requirements

### Requirement: Admin network faults have repeatable process-level release evidence

The scheduled/manual release workflow SHALL repeatedly execute real HTTP/gRPC tests for duplicate batches, sequence gaps, Admin restart with pending work, unavailable Admin shutdown, invalid remote authentication, and SIGTERM while a command ACK is pending. It SHALL require each behavioral test to pass, capture raw output, record each scenario's process-level completion p50/p95/p99 and Linux child peak RSS, and retain a machine-readable result on failure. The report SHALL label these as whole-scenario times including test setup, not request-only latency. Numeric limits SHALL be accepted after baseline measurement and enforced before P06 Task 5 closes.

#### Scenario: The complete fault matrix succeeds

- **WHEN** every named fault scenario is executed repeatedly against real transports
- **THEN** each run preserves its behavior assertions and records duration and process memory without silently omitting a scenario

#### Scenario: A scenario or measurement fails

- **WHEN** an executable is missing, a test fails or times out, output is malformed, or an accepted limit is exceeded
- **THEN** the release workflow fails and retains available raw logs and JSON failure evidence
