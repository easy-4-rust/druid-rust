# PE-02：Druid 原生池标准 pooled owner

## 结果

5.5 已完成。新增 `DruidPooledConnectionOwner`，从 `DruidPool::get_pooled_connection_owner` 或 `DruidDataSource::get_pooled_connection_owner` 取得；现有直接 `get_connection` API 保持不变。Owner 持有单一 Druid pooled connection/holder，并按代际发放至多一个活动标准逻辑连接。逻辑关闭先向固定 owner source 通知一次，再经过现有异步 recycle/reset；可复用 holder 留在 owner。Owner 关闭后拒绝后续借用，使旧逻辑句柄报告 closed，并将干净 holder 返回原生池一次；复位失败将 holder 丢弃。监听器由 `PooledConnectionEventRegistry` 快照分发，不持有 registry/source 锁。

## 测试与验证

- `cargo test -p druid --test standard_pooled_connection_owner_test --locked --offline`：2 passed。覆盖 owner 稳定事件源身份、单物理连接多逻辑租约、logical close 仅结束租约、owner close 活动租约与失效旧句柄、native active/idle、物理连接关闭计数。
- `cargo test -p druid --test task6_recycle_complete_contract_test --locked --offline`：16 passed；覆盖 rollback/reset 顺序、失败 discard、return validation 与取消后回收。
- `cargo test -p druid --test pool_close_backpressure_test --locked --offline`：11 passed；覆盖慢物理关闭、有限关闭队列和 shutdown deadline。
- `cargo test -p druid --test task6_pooled_connection_exception_paths_test_parity --locked --offline`：1 passed。
- `cargo clippy -p druid --test standard_pooled_connection_owner_test --locked --offline -- -D warnings`：通过。
- `cargo fmt -p druid -p druid-rdbc -- --check`：通过；`git diff --check`：通过。
- `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：通过。
- 首次严格 Clippy 发现原有尾表达式和重复分支 lint，已进行语义等价修正；Druid/ RDBC owner 定向 Clippy 最终无 warning。

未运行真实数据库测试：本 owner 生命周期门禁使用受控标准驱动检查 Druid 原生池状态机，不把它表述为数据库后端认证。owner 关闭与逻辑关闭并发由原子租约转换、owner mutex 串行化和旧代际拒绝共同保护；本测试验证 owner 关闭时仍持有逻辑句柄的关键竞态边界。

## 变更源码身份（SHA-256）

```text
crates/druid/src/core/druid_pooled_connection.rs 46D610BAC8D5754A017C87F23ACFFCA5711DE16908318A4E3B0636312A4C8622
crates/druid/src/core/druid_pooled_connection_owner.rs 0E1A818FEC11A77F8BFBE5D09DCF30E0883B236ACE63ED0C9AE3EA94CE0707D2
crates/druid/src/core/standard_logical_policy.rs 6FC6CC136D181E65B845D10C5E1388F88C16BBD06879419A97CB78E7E503DFF7
crates/druid/src/core/mod.rs 9972EBCA13EC72A0472AB9AD89BBD4247D21DA587D92DDAC44C791AC1E95BEDE
crates/druid/src/pool/druid_data_source.rs 6EBC726C2AA27F0BDD9CC980BD8129226769C5FE00F595E8A96147641B3E997E
crates/druid/src/pool/druid_pool.rs D21F77BDDF86F8036DA549DFF01F25B86412C5CFE157046370EA5D35557CA778
crates/druid/src/pool/mod.rs F69C69F8F395DCAD0750909A25478D4DF888FDAD76AFD5C514AD37C08E9CDDF4
crates/druid/tests/standard_pooled_connection_owner_test.rs FD67E2575A7A30774BA419F1E60EC6259BC246679108E089D265014D4D2B5234
crates/druid-rdbc/src/data_source/pooled_connection_event_registry.rs 0B6D15984D5CA6B92FBBE56595E7B24647F837CED9F6E5BA3583DA5200425064
crates/druid-rdbc/src/types/timestamp.rs 585F235AB335E6975A110B47375B2A2692C41420956FF8BAF7FD43B8FD6BF822
crates/druid-wrapper/src/standard_adapter_executor.rs 1894D159A92B80BF4827A85E3B4A77D306AEC3689C4C2C0B71B5D24DF2EA8D99
crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs C7444E1E5E09D5173CA4ADB38CB3854850973D268967DE49E75EC710452FE2DD
crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs 7F83B18B0BF37765B0EF35F93B7F5914D32E75840B6E2674B97A482E52BE1DE0
docs/rdbc/pooled-connection-events.md 8E550B29AD87BB618E14562BD190C132F603B315778F8F6CA3FB9545FF21848F
docs/druid-rust-Architecture.zh_CN.md 68920078F5CA402CA5F9362085C4940BBA986ED023A4488ED2635F4072C8CC3C
```
