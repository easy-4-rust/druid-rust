## ADDED Requirements

### Requirement: CD-08 Dynamic data source supports canonical acquisition

An existing dynamic Druid data source SHALL implement the canonical RDBC `DataSource` and `CommonDataSource` contracts without creating another pool. A standard connection request SHALL use the currently selected data-source group, and connection close SHALL return the holder to its selected pool. Canonical unwrap SHALL expose the same object's standard data-source views. A per-call credential request without a verifier SHALL fail with SQLState `0A000` before borrowing a connection.

#### Scenario: Group switches between standard borrows
- **WHEN** a standard consumer borrows and closes a connection, then the dynamic group switches and the consumer borrows again
- **THEN** the first holder is returned to the old group and the second connection comes from the new group

#### Scenario: Standard inspection and unsupported credentials
- **WHEN** the consumer inspects standard data-source views or requests per-call credentials
- **THEN** inspection returns the same object, while the credential request fails with `0A000` without a borrow
