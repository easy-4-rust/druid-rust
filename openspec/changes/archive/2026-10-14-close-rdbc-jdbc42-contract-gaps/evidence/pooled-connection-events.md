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

### PE-03 实现提交时源码身份

本节哈希记录 PE-03 实现交付时的源码快照。5.4 后续复用了该回归文件扩展顺序与重复注册断言；当前哈希见 PE-05。

| 文件 | SHA-256 |
|---|---|
| `crates/druid/src/core/druid_pooled_connection.rs` | `DC05A23C1BA6B6284A65B74CD34C0BF66E57CC323A3D02FA7FF2DE57061C2275` |
| `crates/druid/tests/task6_druid_pooled_connection_differential_test_parity_part_02.rs` | `C2975BA1F2D9E17265A462CF62C664921198CE42D37E46D1E189DBC1740E0120` |

本节关闭 5.2 的 Druid 产品连接关闭/fatal 错误处置场景。标准 RDBC `PooledConnection` 具体消费者接入及 PreparedStatement/CallableStatement 事件分发仍由 5.3–5.4 验收。

## PE-03：PreparedStatement/CallableStatement 事件

PreparedStatement 与 CallableStatement 现通过 holder 共享的监听器注册表发送语句关闭和 SQL 执行异常事件。事件保留原语句对象身份；监听器列表在回调前快照，锁外逐个调用。移除操作即时影响既有语句；单个回调 panic 被隔离，不吞掉原 SQL 错误或中断关闭清理。逻辑关闭及物理 close 成功路径共用一次性门闩，因此同一语句不会重复发送关闭事件。普通 Statement 不被误标为此项新增的 Prepared/Callable 执行事件能力。

先添加 PreparedStatement 关闭事件回归，基线断言失败（收到 0 次，预期 1 次）；实现后定向套件通过。验证命令及结果：

| 命令 | 结果 |
|---|---|
| `cargo test -p druid --test task6_druid_pooled_connection_differential_test_parity_part_02 -- --nocapture` | 32 passed |
| `cargo test -p druid --test callable_cache_lifecycle_test` | 7 passed |
| `cargo test -p druid --test task6_connection_holder_coverage_test_parity` | 22 passed |
| `cargo test -p druid-rdbc --test standard_event_contract_test` | 8 passed |
| `cargo fmt -p druid -- --check` | exit 0 |
| `openspec validate close-rdbc-jdbc42-contract-gaps --strict` | exit 0 |
| `git diff --check` | exit 0 |

新增/扩展回归覆盖 PreparedStatement/CallableStatement 关闭事件恰好一次、错误事件 SQLState 与错误对象保留、事件携带原语句身份、prepare 后移除监听器立即生效、监听器自移除的回调重入，以及 panic 后仍发送后续通知和执行清理。以上验证仅关闭 PE-03/PE-04 的实现合同；PE-05/任务 5.4 外部消费者文档、示例和全批验收仍未完成。

### 本次源码身份

以下 SHA-256 对应上述实现与测试文件的当前工作区内容：

| 文件 | SHA-256 |
|---|---|
| `crates/druid/src/core/druid_connection_holder.rs` | `63A1930A460CD7AB8EECE77C2E97353CCBEFB6A17BBA097CA6E2135E65BF3918` |
| `crates/druid/src/core/druid_pooled_statement.rs` | `7D7D89FE2512DB49292F46339A80CAEFBACF35ABEF01088A1372D877BA26D314` |
| `crates/druid/src/core/standard_logical_policy.rs` | `A3E14777E4C124B4F54D1CEA014C1231446D1816CB255BDE93C2D7B2EFBC0741` |
| `crates/druid/src/core/druid_pooled_connection.rs` | `D3A809ED53E1641A6C34C825FBBA32F8B78AED5ABEE27A64FF7D4B01ADC9A67B` |
| `crates/druid/src/core/druid_pooled_prepared_statement.rs` | `9DC7043933ADE1B30D1EA63A9046D61C609D4EB24FE392E0785CEC76F0D3EE84` |
| `crates/druid/src/core/druid_pooled_callable_statement.rs` | `7DFA8EC99C74976D58B89C9CB7D661547D83CBDEA1132686A359AB62B41C3F02` |
| `crates/druid/src/core/statement_event_listener.rs` | `6B4DC1B9E3B3A780EE28060D653187316CCC0D3E78826AB0202A20CC18023CD7` |
| `crates/druid/tests/task6_druid_pooled_connection_differential_test_parity_part_02.rs` | `8E1F20BA2057EE78ED5C381F94C8C5809361CDAA6078C3D9C49DAC124AA75BC2` |
| `crates/druid/tests/callable_cache_lifecycle_test.rs` | `944EFFC4319C344BABBA90A1752FB3EADD4CD00ECEC13CAD0103BB2B5DFDD3F6` |

## PE-05：外部池示例、生命周期和顺序验收

新增 [池化连接事件与生命周期](../../../../docs/rdbc/pooled-connection-events.md)，并从 [集成作者指南](../../../../docs/rdbc/integration-author-guide.md) 链接。文档给出外部池通过标准 listener registry 分发四类事件的关键示例，并说明 callback 需在 owner/池锁外运行。生命周期图与产品调用顺序核对如下：

| 触发点 | 可观察顺序 | 验收边界 |
|---|---|---|
| Prepared/Callable 执行 SQL 异常且 fatal | fatal owner 处置 → statement error event → 原错误返回 | 非 SQL 异常不触发 SQL 事件；错误对象仍返回调用者 |
| Prepared/Callable 关闭成功 | statement closed event（一次）→ resource unregister → after-close filter | 关闭重复调用不重复通知；关闭失败不伪报成功 |
| 逻辑 Connection 关闭 | connection closed event（一次）→ reset/recycle 或 discard | 事件只表示租约开始归还，不表示物理连接可借出 |
| 重复注册同一监听器 | 每一注册项各接收一次回调 | 单次 remove 只删除首个匹配项；remaining registration 继续接收 |

实际运行命令均退出码 `0`：

| 命令 | 结果 |
|---|---|
| `cargo fmt -p druid -p druid-rdbc -- --check` | 通过 |
| `cargo test -p druid-rdbc --test standard_event_contract_test` | 8 passed；含 owner/statement/error identity、锁外重入、重复注册/逐次移除和 panic 快照继续 |
| `cargo test -p druid-rdbc --test standard_external_implementor_test` | 1 passed；独立外部 crate 仅依赖 `druid-rdbc` 完成编译与运行 |
| `cargo test -p druid --test task6_druid_pooled_connection_differential_test_parity_part_02` | 32 passed；包含连接/语句重复注册移除后仅一项回调、连接关闭先于归还回调、statement 重入/panic/关闭事件 |
| `cargo test -p druid --test callable_cache_lifecycle_test` | 7 passed；新断言确认 Callable 的顺序为 `statement-error` → `statement-closed` |
| `cargo test -p druid --test task6_connection_holder_coverage_test_parity` | 22 passed |
| `openspec validate close-rdbc-jdbc42-contract-gaps --strict` | valid |
| `git diff --check` | 通过 |

产品层重复语义用例实际注册同一 `Arc` 两次、移除一次，再验证仅保留的注册项收到一次关闭通知；连接 listener 用例也覆盖了相同规则。RDBC registry 测试独立验证重复项逐次触发、逐次移除及未注册移除返回 false。现有实证覆盖规格要求的冻结 Java `CopyOnWriteArrayList` 默认对象相等语义；自定义 Java `equals` 的特殊监听器仍不宣称可直接映射到 Rust trait-object 身份。

### 本批测试源码身份

| 文件 | SHA-256 |
|---|---|
| `crates/druid/tests/callable_cache_lifecycle_test.rs` | `F57B86FCE63178764EDA2FA62662AC2FDD0756252C668F3B51FAF27A84A8E62E` |
| `crates/druid/tests/task6_druid_pooled_connection_differential_test_parity_part_02.rs` | `ED343E4397B053C2AB605CD3BBF812D8DFD0E796D1E8ED2ACDD981158687DE0C` |
