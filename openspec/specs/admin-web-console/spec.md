# admin-web-console Specification

## Purpose
Deliver the Druid Admin console as a separated front end: the Admin server exposes one versioned JSON API and an independently built single-page application consumes it, presenting the same information Java Druid's admin presents — datasource configuration, SQL statistics with parsed detail, Wall statistics, pooled connections and active-connection stack traces — rather than a reduced push-only view. The Java-compatible routes stay as a compatibility layer, and capabilities the platform cannot provide are reported as unsupported instead of being rendered as empty tables.

## Requirements

### Requirement: The Admin serves the Java-equivalent management data

The Admin SHALL serve, for every datasource it can reach, the management information Java Druid's admin serves: the pool configuration and runtime counters, the per-SQL statistics including the parsed detail, the Wall statistics including per-table and per-function rows and the white and black lists, the pooled-connection rows and the active-connection stack traces. A capability that the platform cannot provide SHALL be reported as unsupported with a reason instead of being reported as a zero-valued row.

#### Scenario: A datasource is visible with its configuration

- **WHEN** an operator requests the datasource list
- **THEN** every reachable datasource appears with its identity, name, database type, driver, connection URL, pool sizing, validation flags and runtime counters
- **AND** a datasource that is not reachable does not appear

#### Scenario: A datasource detail is requested

- **WHEN** an operator requests the detail of one datasource by its identity
- **THEN** the response carries that datasource's configuration, counters and histograms
- **AND** an unknown identity is answered with a not-found status rather than an empty success

#### Scenario: A SQL statement is inspected

- **WHEN** an operator requests the detail of one SQL identity
- **THEN** the response carries the statement, its formatted form, the parsed table, fields, conditions, relationships and ordering columns, its execution counters, its histograms and its last error detail
- **AND** an unknown SQL identity is answered with a not-found status

#### Scenario: Wall statistics are inspected

- **WHEN** an operator requests the Wall statistics
- **THEN** the response carries the check, hard-check, violation, syntax-error, white-list and black-list counters
- **AND** the per-table rows with their operation counters, the per-function rows with their invocation counts, and the white-list and black-list entries

#### Scenario: Pooled connections are inspected

- **WHEN** an operator requests the pooled connections of one datasource
- **THEN** the response carries each connection's identity, use count, connect and last-active times, holdability, transaction isolation, auto-commit and read-only flags, keep-alive check count and its prepared-statement cache entries

#### Scenario: Active connection stack traces are requested

- **WHEN** an operator requests the active-connection stack traces of one datasource
- **THEN** the response carries one stack frame list per active connection
- **AND** a datasource that does not track abandoned connections reports the requirement instead of an empty list

### Requirement: The Admin management API has one versioned contract

The Admin SHALL expose its management data under a versioned API namespace whose responses use one field naming convention, one status-code vocabulary and one content type. A missing credential SHALL be answered with an unauthorized status, an unknown identity with a not-found status and a malformed identity with an unprocessable-entity status. The versioned API SHALL publish an OpenAPI document describing every route it serves.

#### Scenario: A client reads a versioned route

- **WHEN** a client requests a versioned management route
- **THEN** the response is JSON with a documented content type
- **AND** every field name follows the versioned contract's convention rather than the compatibility layer's mixed conventions

#### Scenario: An identity is absent or malformed

- **WHEN** a versioned route requires an identity that is absent or malformed
- **THEN** the response carries the corresponding unauthorized, not-found or unprocessable-entity status
- **AND** the response never reflects the raw request value back to the client

#### Scenario: The API contract is published

- **WHEN** the API document is requested
- **THEN** it describes every route of the versioned namespace, its parameters and its response shape
- **AND** it is generated from the same definitions that implement the routes

### Requirement: The management console is a separated application

The Admin SHALL deliver its console as a separately built application that communicates only through the versioned API. The built application SHALL be embedded in the Admin binary so the Admin remains a single self-contained executable, and the API the application consumes SHALL carry no dependency on how the application is delivered. Adding a console asset SHALL NOT require editing Rust source.

#### Scenario: The console is served

- **WHEN** an operator requests the Admin root
- **THEN** the Admin serves the built console application
- **AND** the application loads its data from the versioned API rather than from a server-rendered page

#### Scenario: A static asset is added

- **WHEN** a new asset is added to the console application and the application is rebuilt
- **THEN** the Admin serves it without a change to any Rust source file

#### Scenario: The Rust build has no console application

- **WHEN** the console application has not been built
- **THEN** the Admin still compiles and still serves its API
- **AND** the absence of the console is reported instead of the Admin failing to start

### Requirement: The console authentication supports the separated client

The Admin SHALL authenticate a console client by session cookie when the application is served from the Admin itself, and by a bearer credential when the application is served from another origin. Cross-origin access SHALL be permitted only when the operator configured it. Enabling the versioned API SHALL NOT weaken the rule that a non-loopback Admin requires both TLS and authentication.

#### Scenario: The console is served from the Admin

- **WHEN** a client authenticates with valid credentials and then presents the session cookie
- **THEN** the versioned API serves the management data

#### Scenario: The console is served from another origin

- **WHEN** a client presents a valid bearer credential and the operator enabled cross-origin access
- **THEN** the versioned API serves the management data and permits the configured origin
- **AND** an origin that was not configured is refused

#### Scenario: The Admin is exposed beyond loopback

- **WHEN** the Admin is configured with a non-loopback address
- **THEN** it refuses to start without both TLS and authentication
- **AND** the versioned API does not relax that requirement

### Requirement: The console presents live middleware monitoring
The console SHALL open a dense homepage backed by the versioned management API and retain datasource, SQL, slow SQL, Wall and API features. It SHALL expose collection health and capability boundaries using real observations. It SHALL NOT ship prototype fixtures or demonstration controls.

#### Scenario: A field is not reported
- **WHEN** a source cannot report a metric
- **THEN** the console labels it unavailable and excludes it from complete aggregates
- **AND** neither zero nor an empty healthy table substitutes for missing telemetry

#### Scenario: An operator refreshes a filtered page
- **WHEN** automatic or manual refresh completes
- **THEN** the current filter, sort, page and detail selection survive
- **AND** an older request cannot overwrite a newer result

#### Scenario: Identities collide
- **WHEN** multiple services or datasources share a numeric identity
- **THEN** the console sends source context for detail requests
- **AND** an ambiguous unscoped request returns conflict

#### Scenario: Sources fail
- **WHEN** one source fails and another supplies data
- **THEN** available data and the failed source status are both shown
- **AND** complete failure is not presented as an empty successful snapshot

#### Scenario: Slow statements are inspected
- **WHEN** an operator filters by a cumulative maximum duration
- **THEN** filtering precedes bounded pagination
- **AND** cumulative statement executions are not labelled as slow request percentage

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
