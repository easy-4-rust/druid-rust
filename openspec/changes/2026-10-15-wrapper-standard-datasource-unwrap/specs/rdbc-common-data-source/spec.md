## ADDED Requirements

### Requirement: CD-07 Wrapped ordinary data source exposes its implemented standard views

An ordinary `DruidRdbcDataSource` SHALL report and unwrap canonical `DataSource` and `CommonDataSource` targets as views of the same object. Inspection SHALL NOT create another pool or borrow a connection.

#### Scenario: Standard consumer inspects both interfaces
- **WHEN** a consumer calls `is_wrapper_for` and `unwrap` for either canonical data-source interface
- **THEN** inspection succeeds and the returned view has the original object's identity

#### Scenario: Unsupported target
- **WHEN** a consumer requests a canonical interface the ordinary data source does not implement
- **THEN** `is_wrapper_for` returns false and `unwrap` returns feature-not-supported
