## ADDED Requirements

### Requirement: CD-05 CommonDataSource has one standard trait identity

`druid-rdbc::data_source::CommonDataSource` SHALL be the only `CommonDataSource` trait identity. `druid::sql::CommonDataSource` SHALL remain an exact named re-export for source-path migration and SHALL NOT define another trait or independent method defaults. Druid and wrapper data sources SHALL implement the canonical contract once. The approved pre-release migration replaces the old Druid integer-seconds getter and setter with the standard `try_login_timeout`/`Duration` interface; callers must observe standard unsupported and error results rather than a fabricated numeric default.

#### Scenario: Both public import paths identify one contract
- **WHEN** an application imports `CommonDataSource` from either `druid::sql` or `druid_rdbc::data_source`
- **THEN** both paths expose the same trait identity and method semantics without duplicate implementations

#### Scenario: Dynamic data source has no configured timeout
- **WHEN** a dynamic data source has no explicit login timeout or parent logger
- **THEN** `try_login_timeout` returns `Ok(None)` and `get_parent_logger` returns feature-not-supported with SQLState `0A000`
