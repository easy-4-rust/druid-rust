# rdbc-resource-operations-migration Specification

## Purpose
固定历史资源能力消费者的迁移分母，并以标准 `ResourceOperations` 入口、生产源码和可执行资源合同证明这些消费者已经完成迁移。

## Requirements

### Requirement: Frozen resource capability consumers close against the actual standard type

The 108 historical `RdbcResourceCapabilities` consumer identities SHALL remain intact and map one-to-one to `druid_rdbc::resource::ResourceOperations`. Final Druid evidence SHALL count them closed only when the ledger is exact, the retired name is absent from production, the standard type is exported, and the resource contract passes.

#### Scenario: Complete standard migration

- **WHEN** all 108 historical identities map to the standard type, the retired name is absent from production, and the resource contract passes
- **THEN** the Druid consumer evidence reports zero Task 7 resource-operation consumers pending
- **AND** the historical inventory and unrelated Task 7 obligations remain unchanged

#### Scenario: Closure evidence drifts

- **WHEN** an identity is omitted, duplicated, fabricated, or the old symbol reappears in production
- **THEN** verification fails before reporting the consumers closed
