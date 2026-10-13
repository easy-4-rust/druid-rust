# XA 标准合同证据（6.1）

## 交付

- `druid-rdbc::data_source` 是 XA 协议类型的唯一标准来源：`Xid`、`XaFlags`、flags 模块、`XaPrepareResult`、`XaErrorCode`、`XaError` 和 `XaResource` 均各自位于独立源文件。
- `XaConnection::xa_resource` 提供 owner 上的资源访问入口。默认实现带结构化 XA 错误及 SQLState `0A000` 来源链，保留旧外部 `XaConnection` 实现的源兼容性。
- `druid::core::xa` 保留产品状态机，并重导出标准类型；`druid` 不再重复定义 XA 协议资源、XID、flag 或投票类型。
- 外部集成测试仅依赖 `druid-rdbc`，从标准 `XaDataSource` 获取 `XaConnection` 后调用资源协议；另外验证旧实现不声明 XA 时明确失败，以及超长 XID 返回 `XAER_INVAL`。

## TDD 与验证

- 变更前的外部消费者测试因 `XaResource`、`Xid`、flags、prepare 结果、结构化错误以及 `XaConnection::xa_resource` 缺失而编译失败。
- `cargo test -p druid-rdbc --locked --offline`：通过；包含独立外部消费者集成测试和 doctests。
- `cargo test -p druid-rdbc --locked --offline --test xa_standard_external_consumer`：3 passed。
- `cargo test -p druid --locked --offline --test xa_state_machine_test`：54 passed；覆盖 Druid 原状态机和 XID 兼容行为。
- `cargo check -p druid --example xa_demo --locked --offline`：通过；现有产品示例通过标准重导出实现 XA trait。
- `cargo fmt -p druid-rdbc -p druid -- --check`：通过。
- `python scripts/verify_rdbc_single_standard_purity.py --phase final`：通过，`STANDARD_FILES=127`；无产品反向依赖，协议标准仍只有一份。
- `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：通过。

这些结果关闭标准 API 和源兼容合同，不代表已接通 Druid 真实 XA owner、recover 扫描或真实数据库恢复；相应行为仍由 6.2–6.5 验收。

## SHA-256

```text
crates/druid-rdbc/src/data_source/mod.rs 35208d36de74247f802d6459756ab70872b9bfb9c7d28a769d292006f9cf438f
crates/druid-rdbc/src/data_source/xa_connection.rs 070aa62647d20a3d7a89e088ae39c1d782771e8fbccc908250bdf03114242c79
crates/druid-rdbc/src/data_source/xa_error.rs bfa5819de746f03509377447d2984a932f5534b706008ccfa916734fab05907b
crates/druid-rdbc/src/data_source/xa_error_code.rs f74600a535c9dd6662a9b5e5b6fdea15bca1d37578dac2606cbaa0234805dbe6
crates/druid-rdbc/src/data_source/xa_flags.rs c21061179261012e49e8c390b30252af1836fea11c1096c47a7b883222c804a0
crates/druid-rdbc/src/data_source/xa_prepare_result.rs 5534d84b065a0428e450e3541a0e909f81d775f18b8f08260db7ec2bcf937382
crates/druid-rdbc/src/data_source/xa_resource.rs 17b88fd6a5d91b4527c109104509972b65c985bf2f293968e97337d78e585a03
crates/druid-rdbc/src/data_source/xid.rs 1bd1e6e4cc507426f3854ca16e325cb8ad8214ad9d9462de86102eb331bb3a49
crates/druid-rdbc/tests/xa_standard_external_consumer.rs bde8ba36559384c5d79a5be3990a9ea35819f946a8c1ec27ffa173a0677f4007
crates/druid/src/core/xa.rs c099402dc0545bd48af7d4fd72c1cb3a8ef1415e68a8e4760ab1cd07b0a05c11
crates/druid/src/core/mod.rs 0d142517128bac9302321e009718b06978244df1713874e3ee748b58cb830693
crates/druid/examples/xa_demo.rs eec0631f2814aa3e226ccf60974d46fed334cf2225d3ddf66e3dbc6b7fba0a25
```
