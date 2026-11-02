## ADDED Requirements

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
