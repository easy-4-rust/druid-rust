# CD-01–04：CommonDataSource 配置示例与迁移

## 结果

7.4 已完成。新增 [CommonDataSource 配置与迁移指南](../../../../docs/rdbc/common-data-source.md)，覆盖普通 RDBC wrapper、池化数据源、XA 数据源、writer 创建、可失败登录超时读取、已构建池的不可变配置边界，以及旧 getter/产品兼容 trait 的迁移差异。超时迁移页和 XA 集成指南现指向该总指南。

公开消费者测试发现一个真实遗漏：`DruidPoolBuilder::login_timeout(37)` 构建的数据源通过标准 `try_login_timeout()` 返回 `None`。测试首次失败时观测到 `left: None`、`right: Some(37s)`。已为 `DruidDataSource` 实现标准 getter，并让 `DruidRdbcDataSource` 委托底层读取；负秒值返回 `InvalidArgument`，不伪装成默认值。零秒暴露为 `Some(Duration::ZERO)`，符合合同中的无限等待语义。已构建池的 setter 继续返回 `UnsupportedOperation`，已有配置保持不变。

## 外部消费者与回归验证

- `cargo test -p druid-wrapper --features jdbc-agent --test common_data_source_public_examples_test --locked --offline`：1 passed。使用公开 builder、标准 CommonDataSource、普通 wrapper 和 XA 数据源 API 验证池/普通 getter、writer 配置与身份、零值、负值错误、构建后 setter 和 XA 父命名空间。该消费者不建立物理数据库连接。
- `cargo test -p druid-rdbc --test common_data_source_timeout_contract_test --locked --offline`：2 passed，覆盖旧实现默认适配、零值、最大时长及原始读写错误保真。
- `cargo test -p druid --test common_data_source_trait_test --locked --offline`：13 passed，产品兼容 CommonDataSource 行为保持。
- `cargo test -p druid --test common_data_source_logging_pool_test --locked --offline`：4 passed。
- `cargo test -p druid-wrapper --test common_data_source_logging_test --locked --offline`：2 passed。
- `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`：2 passed，3 项真实 Agent/数据库用例按其环境要求标记 ignored；未将 ignored 计为通过。
- `cargo test -p druid-rdbc --locked --offline`：完整 crate 测试与文档测试退出码 0，其中冻结方法级测试二进制 435 passed。
- 修改包的 `cargo fmt --check`、`git diff --check` 和 `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` 均通过。
- `cargo clippy -p druid-rdbc -p druid -p druid-wrapper --lib --features druid-wrapper/jdbc-agent --locked --offline -- -D warnings -A clippy::semicolon_if_nothing_returned -A clippy::match_same_arms -A clippy::type_complexity -A clippy::unnecessary_map_or -A clippy::await_holding_lock -A clippy::manual_assert_eq -A clippy::cloned_ref_to_slice_refs`：库目标通过；命令显式豁免此前记录的仓库存量 lint 类别，未声称全 targets 无 lint。

## 迁移与行为边界

- 新代码调用 `try_login_timeout()` 并处理 `Result`；弃用的 `login_timeout()` 仅保留旧实现兼容，不应继续用于能观察底层读取失败的实现。
- `druid_rdbc::data_source::CommonDataSource` 以 `Duration` 表达标准可失败配置；`druid::sql::CommonDataSource` 是整数秒产品兼容接口，两者是不同 trait。重名方法须显式限定 trait。
- Druid pool 的登录超时在 builder 阶段设置。建池后调用标准 setter 返回错误，不会仅更新 getter 展示值。
- XA 超时由厂商 XADataSource 支持的创建属性决定；属性名和单位不由通用合同猜测。writer 仍由 XA 数据源实例提供。
- 本项验证配置合同和公共 API 示例，不声称连接池成功物理建连，也不替代 XA/数据库真实集成测试。

## 源码身份（SHA-256）

```text
crates/druid/src/sql/data_source.rs 0EF9E9BCC8755D2E97E418467B1A51A5B8557F6778EBF86157BE3FB9BBFDF901
crates/druid-wrapper/src/rdbc/druid_rdbc_data_source.rs E827472E14C88077179B5E11037261F5631233EB453882ADC4F5C0B89BB8B1AA
crates/druid-wrapper/tests/common_data_source_public_examples_test.rs 0D109EEC9D6A0EE586293769B120B4183FADDCCE91C515C08B4CD81924FE9C40
docs/rdbc/common-data-source.md 4D5E726B544C35BA2791591AAD58679024D77F20A8EFA3130817972AEA7802D0
docs/rdbc/common-data-source-timeout.md 45496AFA56A9644E548D3C05302D690A5BCC72A0D0CC75DD2A51D3C673434D5F
docs/jdbc-agent-xa.md D7FFF2D3DFF8BA874CA47063555B063D4682626FA9AA74B07826DD498085F54C
openspec/changes/close-rdbc-jdbc42-contract-gaps/tasks.md A0C2F5216D61950EF4FA4BF449197507C0E123788DFF3FF0B0C2B09702E2B93D
```
