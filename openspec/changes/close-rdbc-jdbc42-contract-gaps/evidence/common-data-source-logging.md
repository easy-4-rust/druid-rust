# CD-02/03：公共日志入口与父命名空间

## 结果

7.2 已完成。CodeGraph 确认此前存在两个彼此独立的 `CommonDataSource`：标准层 `druid-rdbc` 只有登录超时合同；产品兼容层 `druid::sql` 已定义 `RdbcLogWriter` 和日志入口，`DriverManager` 持有独立进程级 writer。实现将 `RdbcLogWriter` 类型移至标准层，再从 `druid::sql` 重导出，保留旧公开路径和底层类型。普通 `DruidRdbcDataSource`、池化 `DruidDataSource`、XA `JdbcAgentXaDataSource` 均实现标准日志配置和父命名空间合同；XA 数据源克隆共享同一配置状态。

## 合同与测试

- TDD RED：新增 `common_data_source_logging_contract_test.rs` 后，首次运行 `cargo test -p druid-rdbc --test common_data_source_logging_contract_test --locked --offline` 因标准 trait 缺少 `RdbcLogWriter`、`get_log_writer`、`set_log_writer`、`get_parent_logger` 编译失败（E0432/E0407/E0599）。
- GREEN：外部消费者合同测试验证普通 `DataSource`、`ConnectionPoolDataSource`、`XaDataSource` 三类实现可经共同 supertrait 读取、设置和禁用 writer，并验证不支持父命名空间返回 SQLState `0A000`：2 passed。
- 真实池化 Druid 数据源测试验证初始空值、写入器身份保持、标准和旧 `druid::sql` API 共享同一 writer、父命名空间和禁用：1 passed。
- 真实普通 RDBC wrapper 测试验证 `DruidRdbcDataSource` 使用标准 CommonDataSource，writer 设置/读取/禁用及 `druid.pool` 父命名空间：1 passed。
- `JdbcAgentXaDataSource` 测试验证初始空值、clone 间共享、禁用和稳定父命名空间：1 passed。
- `cargo test -p druid-rdbc --locked --offline`：退出码 0；完整 crate 集成测试、外部实现测试及文档测试通过，包括方法覆盖二进制 435 passed。
- `cargo test -p druid --test common_data_source_trait_test --locked --offline`：13 passed，旧产品兼容合同仍通过。
- `cargo check -p druid-wrapper --features jdbc-agent --locked --offline`：退出码 0。
- 定向测试、逐 crate `cargo fmt --check`、`git diff --check` 与 `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` 均通过。

曾把真实连接获取临时加入 wrapper 日志配置测试，但该测试夹具在连接操作处超过 60 秒未结束，因此停止该额外探针并移除连接获取步骤；本功能点只验证 CommonDataSource 配置，不以此测试声称连接获取行为。既有池连接测试仍负责连接生命周期验证。

## 源码身份（SHA-256）

```text
crates/druid-rdbc/src/data_source/common_data_source.rs 4F093EF5526648FA8D7ADA986FB9392185716EF46067FFFAC17F6F8E9C10B84E
crates/druid-rdbc/src/data_source/mod.rs 1B87BEA09A0841031DAF2ED5A98ED21F1EC26D560D7B788B8F9EDD2DDD2279EA
crates/druid-rdbc/tests/common_data_source_logging_contract_test.rs 060D0D4536C0AF477066E5F17567579886B2EC23DB19AC4F244ED5AF6775DA20
crates/druid/src/pool/druid_data_source.rs 06CA14F1884D01AE0AB7EE752987527945EEE7155A730A595F6D9A3AAA5E14F1
crates/druid/src/sql/common_data_source.rs 3FE82329E4109508AE2209376F5D8781CF9F45E42BF8F22F6E898E46C5E97D19
crates/druid/src/sql/data_source.rs 323C192CA67EECB26A5F376F452FACA9AF42E25F12639D1E7EC3BA4B93EC13D8
crates/druid/tests/common_data_source_logging_pool_test.rs 318EFE8BAEC17F83D31301F8E2E1B164BF6BE7CB33A7A652A1E5EAC21DBAE0CA
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_data_source.rs 8637746283514B0F49CE0E1A3CF7DD5E98A78E43DC5F94FF1971C79FB3A59746
crates/druid-wrapper/src/rdbc/druid_rdbc_data_source.rs 53DCD565262208D6C660F66FDB1C037A6306EC4C1C66A1B75D15A75294C9B6C2
crates/druid-wrapper/tests/common_data_source_logging_test.rs 3BA5B864DC74E4650EA869159FA8D96F45AAEBCFEF5FA8845A066AB07D604871
Cargo.lock 96386C1D43987D86CF1E98475DB1D50F0A5FC837A3027A8FCC34FE301523CCBF
crates/druid-rdbc/Cargo.toml B93456C159A97D11E58934051E3EE83BA3ED0F6D490DF100D52A4DD16B68E3B2
```

## 边界

此证据关闭 CD-02/03 的公开配置入口和父命名空间。数据源 A/B 与全局 `DriverManager` 的事件隔离、实际诊断输出接入、凭据脱敏以及日志 I/O 故障与 SQL 错误优先级留在 7.3/CD-04；不支持的可选 pooled provider 继续由默认合同明确报不支持。
