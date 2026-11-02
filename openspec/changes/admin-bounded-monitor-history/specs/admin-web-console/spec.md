## ADDED Requirements

### Requirement: The Admin retains bounded recent monitoring samples

The Admin SHALL retain recent numeric samples from accepted push snapshots with a fixed total capacity and TTL, expose them through an authenticated versioned read API, and show their source and retention limits in the console.

#### Scenario: Accepted samples are queried
- **WHEN** a service sends accepted datasource snapshots
- **THEN** the operator can inspect recent pool, SQL and Wall counters under the service and datasource identity
- **AND** the query is bounded and includes client sampling time and server receive time

#### Scenario: Counters reset or a submission is rejected
- **WHEN** a cumulative counter falls or a batch is rejected
- **THEN** a reset has an unavailable delta and a rejected batch contributes no history

#### Scenario: History reaches capacity or TTL
- **WHEN** the sample bound or expiry is reached
- **THEN** the oldest or expired samples are removed without weakening latest-state limits
- **AND** the console states that recent history is process-local and does not survive restart

### Requirement: The Admin reports observed process memory in collection health

The Admin SHALL sample its own resident physical memory for the collection-health API. If sampling is unavailable, it SHALL report an unavailable value and SHALL NOT substitute the repository's serialized-state size.

#### Scenario: Process memory can be sampled
- **WHEN** the operator reads collection health and the process memory sampler succeeds
- **THEN** the response contains the observed process resident bytes

#### Scenario: Process memory sampling fails
- **WHEN** the process memory sampler cannot report a value
- **THEN** the response contains null and marks the field unavailable
