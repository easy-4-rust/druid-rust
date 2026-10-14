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

## XA 协议状态机（6.2）

### 实施与边界

- `XaTransactionState` 增加 suspend、resume、join 转换；一阶段提交只允许在 end 之后、prepare 之前执行；非法转换先校验再改变状态，失败时保留状态和历史。
- 示例 `InMemoryLedger` 按完整 XID（format ID、gtrid、bqual）隔离分支，实现 start/end/prepare/commit/rollback/forget 的标准协议路径。覆盖 join/resume、suspend、TMFAIL 回滚投票、read-only 投票移除分支以及未知/重复/非法操作的错误码。
- XA 常量以冻结 JDK 8 `javax.transaction.xa.XAResource` 为基准：TMNOFLAGS=0、TMJOIN=0x00200000、TMRESUME=0x08000000、TMSTARTRSCAN=0x01000000、TMENDRSCAN=0x00800000、TMSUCCESS=0x04000000、TMFAIL=0x20000000、TMSUSPEND=0x02000000、TMONEPHASE=0x40000000、XA_OK=0、XA_RDONLY=3。JDK 8 XAResource 没有 TMASYNC；已删除此前非标准的同名常量/断言。
- 该内存 ledger 只证明 XA 协议桥接与状态机，不执行数据库 I/O，也不证明 crash recovery。recover 扫描语义、owner 生命周期和真实 JDBC Agent/XA 数据库闭环仍由 6.3–6.4 验收。

### 验证

- `cargo fmt -p druid-rdbc -p druid -- --check`：通过。
- `cargo test -p druid --example xa_demo --locked --offline`：8 passed。
- `cargo test -p druid --test xa_state_machine_test --locked --offline`：54 passed。
- `cargo test -p druid-rdbc --locked --offline`：通过，含外部 XA consumer 和 doctests。
- `cargo run -p druid --example xa_demo --locked --offline`：退出码 0，四个示例场景完成。
- `python scripts/verify_rdbc_single_standard_purity.py --phase final`：通过，`STANDARD_FILES=127`。
- `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：通过。

数值来源：[OpenJDK 8 XAResource.java](https://code.googlesource.com/edge/openjdk/%2B/refs/heads/master/corba/src/share/classes/javax/transaction/xa/XAResource.java)。

### SHA-256

```text
crates/druid-rdbc/src/data_source/xa_flags.rs ca5ea24dab46ee1bea393030bf0cced39b29a0a91d92263fb78fc55dcc8a18c0
crates/druid-rdbc/src/data_source/xa_error_code.rs b07b0852bf4dbb324e5185f4b448ccc8d7d15012166273a04aa31004a594e1c2
crates/druid/src/core/xa.rs d63418481ad1be8b733f5d83e81a50570b01db9583075e07ad920d3836604494
crates/druid/examples/xa_demo.rs c68dc80179fcd794762877bacea6d0706936bb472225063683a6c9a302018f26
crates/druid/tests/xa_state_machine_test.rs 89b3fafa448aed3b1b6d96af53a78c952640d0c06805bd383d140c7b2ec5323f
```

## XA 恢复扫描、RM 身份与 owner 失效（6.3）

### 实施与边界

- 恢复扫描在 `TMSTARTRSCAN` 时固定 prepared Xid 快照，按页继续；非法 flags、无活动扫描时继续/结束、活动扫描重复开始均返回 XA 错误，不破坏有效扫描。`TMSTARTRSCAN | TMENDRSCAN` 一次性模式返回全部可恢复 Xid。
- 测试验证活动分支不进入恢复结果、分页时新增的 prepared 分支不混入旧快照、完整 Xid 无重复/遗漏，以及非法 flags 后有效扫描仍能继续。
- 新增 `XaResourceManagerIdentity` 作为不透明共享令牌；外部消费者可克隆令牌识别同 RM，不同令牌不相同。兼容既有 `XaResource` 外部实现的默认身份方法返回 `None`；示例仅对同一资源对象按数据指针识别，不根据同名或 URL 推断不同对象同 RM。
- 内存示例拒绝超时设置并保持读取值 `0`；`InMemoryXaConnection` 实现标准 `PooledConnection`/`XaConnection`，其 close 会使所持 ledger 资源失效。owner 关闭后 start/end/prepare/commit/rollback/forget/recover/isSameRM/timeout 均返回 `XAER_RMFAIL`，资源入口拒绝再次借出，分支不被改写。
- **范围限制：**该证据验证 RDBC 标准身份类型和示例协议行为。CodeGraph 未发现 JDBC Agent 或其他真实 XA 后端实现；本项不证明物理 XA owner 绑定、数据库恢复或崩溃恢复。JDBC Agent prepare→重建连接→recover→commit/rollback 仍是 6.4，当前记为 `NOT_RUN`，不能据本节宣称 XA 生产就绪。

### TDD 与验证

- `unknown_rm_identity_is_never_inferred_from_object_identity` 的首轮断言失败，暴露同一资源对象的 fat-pointer 比较不稳定；改为比较 trait object 数据地址后通过。不同对象即使名称相同仍返回 false。
- `cargo test -p druid --example xa_demo --locked --offline`：16 passed。
- `cargo test -p druid-rdbc --test xa_standard_external_consumer --locked --offline`：4 passed；覆盖不重写默认 identity 方法的旧外部实现和公开身份令牌。
- `cargo test -p druid-rdbc --locked --offline`：通过（包括外部实现、公共 surface、方法证据测试及 doctests）。
- `cargo test -p druid --test xa_state_machine_test --locked --offline`：54 passed。
- `cargo fmt -p druid-rdbc -p druid -- --check`：通过。
- `python scripts/verify_rdbc_single_standard_purity.py --phase final`：通过，`STANDARD_FILES=128`。
- `openspec validate close-rdbc-jdbc42-contract-gaps --type change --strict --no-interactive`：通过。

### SHA-256

```text
crates/druid-rdbc/src/data_source/mod.rs b7c17f70af86a419b6049dbab1c4a1ec1a5f62c20ff47c979c2a45df61ab1761
crates/druid-rdbc/src/data_source/xa_resource.rs 355ed9581e184f3039b625cd969650d9160f98a5b30cb27bf06a706dbd7fa81a
crates/druid-rdbc/src/data_source/xa_resource_manager_identity.rs 1c16589355861367a0c3876079175d99b2a2d9c0b7b4814a02ebde8176408642
crates/druid/src/core/mod.rs 3538ae156a62e868301b7c264970332b7540e214f034e47729f58218e8231cf7
crates/druid/src/core/xa.rs bbc506bcfb40371152724a6e60acc8ef540d53a0864147ceb68bd2eb2401760d
crates/druid/examples/xa_demo.rs f2d37043a1c0781204f5664286ab9b7c587be42584d3d578226a88dc2f2d5e83
crates/druid-rdbc/tests/xa_standard_external_consumer.rs fbc8d78c156925f21452f8b149b6adadc3067517e0821fe85c3189ff36b46e6f
```
