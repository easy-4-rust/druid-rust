# 池化连接事件证据

## PE-01：标准注册入口

冻结来源为 Alibaba Druid `1.2.28`：[`DruidPooledConnection.java`](https://github.com/alibaba/druid/blob/1.2.28/core/src/main/java/com/alibaba/druid/pool/DruidPooledConnection.java) 将四个连接/语句监听器方法委托到 holder 的监听器列表；[`DruidConnectionHolder.java`](https://github.com/alibaba/druid/blob/1.2.28/core/src/main/java/com/alibaba/druid/pool/DruidConnectionHolder.java) 使用 `CopyOnWriteArrayList`。列表允许重复注册，`remove(Object)` 删除首个相等项；普通 Java 对象默认按身份比较，重写 `equals` 的监听器按其 `equals` 比较。Rust 标准 trait 的当前合同按 `Arc` 身份比较，重复的同一 `Arc` 可逐次删除，未注册项返回 `false`。自定义 Java `equals` 没有直接的 Rust trait-object 等价物，此边界不伪称完全映射。

`PooledConnection` 新增四个同步操作：添加/移除连接事件监听器和语句事件监听器。方法有默认实现，返回相应操作名的 SQLState `0A000`，以保持旧外部实现的源兼容；提供事件能力的池实现可以覆写这些方法。移除方法返回是否移除了一个注册项。外部实现集成测试只依赖 `druid-rdbc`，不访问 Druid holder 或其他产品类型，并在运行时验证两类监听器均可重复注册、逐次移除及对未注册项返回 `false`。

## 实际验证

先运行外部实现测试，确认它因 trait 缺少四个方法而编译失败；添加标准方法后，同一测试通过。当前源码的验证命令：

```powershell
cargo fmt -p druid-rdbc -- --check
cargo test -p druid-rdbc --locked --offline
```

两条命令均退出码 `0`。`druid-rdbc` 完整离线测试套件和 doc tests 通过。Windows 上 `cargo fmt --all -- --check` 因系统路径长度限制报错；按目标 package 执行的格式检查通过。

## 源码身份

在提交前计算的 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-rdbc/src/data_source/pooled_connection.rs` | `4793EBCD1F236CD86504388DA3FDC1DAE3214A18D6141375B2A874EC781BA99A` |
| `crates/druid-rdbc/tests/standard_external_implementor_test.rs` | `11A005381A60B2536EB77DBED4BA395E69580E4AA761D993FA0874566975ABA7` |

本证据仅关闭 PE-01 的标准注册 API 和外部实现合同。它不证明连接关闭/错误通知、语句事件分发、锁外快照或 panic 清理已完成；这些属于 5.2–5.4。

## PE-02：Druid 逻辑连接通知与 owner 处置

Druid 产品层通过 holder 快照向当前注册的连接监听器发送关闭/SQL 错误事件。CodeGraph 路径核验确认 SQL 异常在 `DruidPooledConnection::classify_result_with_sql` 和 `finish_standard_policy_result` 中通知监听器后继续错误分类；fatal sorter 会将 owner disposition 标记为 discard。逻辑关闭经 `close_logical_once` 的 `connection_closed_notified` 门闩通知一次，然后执行回收；重复关闭等待/复用同一个 return completion，不重复执行归还回调。

本次发现并修复监听器 panic 可中断调用方清理的缺口：关闭及连接错误回调各自以 `catch_unwind` 隔离，并通过 `tracing::error!` 记录连接 ID 和回调类型。单个监听器 panic 不阻止后续监听器、fatal sorter/owner discard 或 pooled connection 回收。`panic=abort` 仍会终止进程，不声称可恢复。

### 实际验证

先运行关闭 panic 回归并确认失败：`cargo test -p druid --test task6_druid_pooled_connection_differential_test_parity_part_02 pooled_connection_listener_panic_does_not_skip_recycle_or_duplicate_close_event -- --exact`，退出码 `1`，panic 从 `connection_closed` 回调逸出，归还断言未执行。添加隔离后同一用例退出码 `0`；错误回调 panic 的 fatal 处置用例退出码 `0`。

以下定向命令均退出码 `0`：

```powershell
cargo fmt -p druid -- --check
cargo test -p druid --test task6_druid_pooled_connection_differential_test_parity_part_02
cargo test -p druid-rdbc --test standard_event_contract_test
cargo test -p druid-wrapper --features sqlx-bb8 --test sqlx_bb8_pool_test bb8_fatal_disposition_poison_rejects_transaction_dirty_connection
cargo test -p druid-wrapper --features sqlx-deadpool --test sqlx_deadpool_pool_test deadpool_fatal_disposition_poison_rejects_transaction_dirty_connection
```

两个外部池集成用例各自验证 fatal disposition 后连接被丢弃，并由新物理连接完成后续查询；这与新增回归共同覆盖 bb8 和 deadpool 的“失败连接不再借出”处置路径。RDBC 注册表合同测试另行核对原 owner 与 `Arc<SqlException>` 身份保留。

### 本次源码身份

| 文件 | SHA-256 |
|---|---|
| `crates/druid/src/core/druid_pooled_connection.rs` | `DC05A23C1BA6B6284A65B74CD34C0BF66E57CC323A3D02FA7FF2DE57061C2275` |
| `crates/druid/tests/task6_druid_pooled_connection_differential_test_parity_part_02.rs` | `C2975BA1F2D9E17265A462CF62C664921198CE42D37E46D1E189DBC1740E0120` |

本节关闭 5.2 的 Druid 产品连接关闭/fatal 错误处置场景。标准 RDBC `PooledConnection` 具体消费者接入及 PreparedStatement/CallableStatement 事件分发仍由 5.3–5.4 验收。
