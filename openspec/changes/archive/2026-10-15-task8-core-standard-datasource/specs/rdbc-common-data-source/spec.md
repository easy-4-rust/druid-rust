## ADDED Requirements

### Requirement: CD-06 Native Druid data source exposes the canonical acquisition contract

An existing native Druid data source SHALL be usable as `druid-rdbc::DataSource` without constructing another pool. Standard acquisition SHALL return an owned logical connection with the same close and recycle behavior as the product entry. Canonical `Wrapper` inspection SHALL identify the same data source object. If the core data source cannot verify per-call credentials, the standard credential overload SHALL return feature-not-supported with SQLState `0A000` before borrowing a connection.

#### Scenario: Standard consumer borrows and closes a pooled connection
- **WHEN** an external consumer holds a native Druid data source through the canonical `DataSource` trait and requests a connection
- **THEN** it receives a usable logical connection; closing that connection returns its holder to the same native pool

#### Scenario: Canonical wrapper identity
- **WHEN** a consumer asks the native data source whether it wraps the canonical `DataSource` interface and unwraps it
- **THEN** both operations succeed and the unwrapped view identifies the same object

#### Scenario: Unsupported per-call credentials
- **WHEN** a consumer calls the canonical credential overload on a native data source without a credential verifier
- **THEN** the call fails with SQLState `0A000` and does not borrow or create a connection
