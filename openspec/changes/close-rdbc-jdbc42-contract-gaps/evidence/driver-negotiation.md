# 驱动协商批证据（2026-10-14）

本批实现对应 DN-01–03 和任务 2.1–2.4。

## 行为变化

- `DriverRegistry::connect` 在注册顺序上直接调用每个 driver 的 `connect`，不以 `accepts_url` 预筛，也不查询 `name`。`Ok(None)` 和 `RdbcError::SqlException` 继续尝试；成功立即返回；所有 SQL 异常均失败时返回首个异常原对象；其他 RDBC/内部错误立即传播。
- `first_accepted_driver` 仍仅用于查找，并按注册顺序查询 `accepts_url`。
- `DriverManager` 继续在第三方回调前取得驱动快照，因此回调中注册的驱动从下一轮连接开始可见。登录总期限包装整个协商 future；测试证明超时会丢弃等待中的协商且不开始后续驱动。
- 无合适驱动使用 SQLState `08001`，错误消息不回显原 URL，避免泄露 URL 凭据。
- `Driver::connect` 文档写明拒绝 URL 返回 `Ok(None)`；`Driver::accepts_url` 的查找职责与连接尝试职责分开。

## RED / GREEN

新增 `driver_registry_negotiation_contract_test.rs` 初次运行时出现两项目标失败：首个 `connect` 被 `accepts_url` 过滤掉；首个 SQL 异常使协商提前退出，后续成功驱动未被调用。加入实现后相同四项合同测试全部通过。用例还验证 SQLState `HYT00` 作为单驱动 SQL 异常时继续协商、多个失败保留第一个异常的 code/state/message/next 链、成功后短路、不查询 driver 名称及查找不连接。

DriverManager 及调用方回归实测：

| 命令 | 结果 |
|---|---|
| `cargo test -p druid-rdbc --test driver_registry_negotiation_contract_test --locked --offline -- --nocapture` | 4 passed |
| `cargo test -p druid --test driver_manager_missing_driver_sqlstate_test --locked --offline -- --nocapture` | 1 passed |
| `cargo test -p druid --test driver_manager_login_timeout_test --locked --offline -- --nocapture` | 1 passed；总登录期限到期后第二个驱动调用数为 0 |
| `cargo test -p druid --test driver_manager_registry_snapshot_test --locked --offline -- --nocapture` | 1 passed；回调内注册的新驱动仅在下一次连接可见，重入在 2 秒保护期内完成 |
| `cargo test -p druid --test standard_driver_action_test --locked --offline -- --nocapture` | 6 passed；含注册回调重入及 URL 脱敏 |
| `cargo test -p druid-wrapper --features sqlx --test task6_driver_registry_test_parity --locked --offline -- --nocapture` | 16 passed；含真实 SQLx/SQLite 路由 |
| `cargo fmt -p druid-rdbc -p druid -- --check` | 通过 |

实现前 CodeGraph 确认连接路径为 `DriverManager::get_connection → connect_registered → DriverRegistry::connect`。修改后 CodeGraph 显示 DriverManager 4 个直接注册表连接调用点和新增的针对性测试；现有产品 driver catalog / SQLite 路由目标也通过。

## 未由本批宣称

本批没有执行整个 workspace / 所有可选 feature / 数据库矩阵，也没有提供第三方 JDBC TCK 认证。整体期限通过 Rust future 取消停止本轮协商；底层已发给数据库的操作是否同时中止仍由对应 driver 行为决定，不计入本批能力声明。异常克隆由枚举值保持现有 cause、SQLState、vendor code 和 next 链；当前测试直接构造并比较其公开可构造字段。
