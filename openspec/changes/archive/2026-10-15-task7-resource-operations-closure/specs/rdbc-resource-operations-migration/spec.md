## ADDED Requirements

### Requirement: Frozen resource capability consumers close against the actual standard type

The 108 historical `RdbcResourceCapabilities` consumer identities SHALL remain intact in the frozen inventory and SHALL have an exact one-to-one closure record targeting `druid_rdbc::resource::ResourceOperations`. The final Druid evidence SHALL reject missing, duplicated, invented, or stale closure records. It SHALL reject the retired identifier in production Rust source and require the canonical standard type/export and successful standard resource behavior tests before counting these 108 consumer rows closed.

#### Scenario: Complete standard migration

- **WHEN** all 108 historical identities map to the standard type, the retired name is absent from production, and the resource contract passes
- **THEN** the Druid consumer evidence reports zero Task 7 resource-operation consumers pending
- **AND** the historical inventory and unrelated Task 7 obligations remain unchanged

#### Scenario: Closure evidence drifts

- **WHEN** an identity is omitted, duplicated, fabricated, or the old symbol reappears in production
- **THEN** verification fails before reporting the consumers closed
