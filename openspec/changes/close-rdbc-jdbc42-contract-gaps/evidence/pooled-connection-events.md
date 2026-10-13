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
