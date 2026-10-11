# P05 驱动解析基线

状态：未收敛。基线为 `bb1bea1`，以下是源码路径，不代表三条入口已具备相同运行语义。

| 入口 | 当前路径 | 工厂来源 |
| --- | --- | --- |
| `DriverManager::get_connection` | 复制进程内显式注册的 `Driver`，组装 `druid_rdbc::driver::DriverRegistry`，调用 `connect` | 调用方注册的 `Driver`；未自动安装产品驱动 |
| `lookup_driver_extension("sqlite")` | Core 的 `inventory` 扩展查找 | Wrapper 的 `ToastyConnectionFactory` 异步构造器 |
| `DruidDatabasePoolBuilder::build` | `DruidDriverRegistry::builtin` → `resolve` → `DruidPool::builder` | `SqlxConnectionFactory`（SQLite 档案的 `Sqlx` 模式） |

同一 SQLite 产品目前有独立的 Toasty 扩展和 SQLx 产品建池工厂；`DruidDriverRegistry::resolve` 直接实例化具体工厂，没有向标准 `DriverRegistry` 查询。`DriverManager` 的连接操作已委托标准 registry，但其显式注册 overlay 保留 Java 兼容语义。

`driver_extension_registry_test::factory_is_async_runtime_safe` 在当前提交的 Windows Tokio 测试中通过；旧计划所述同步 `block_on` 陷阱已不适用。`docs/rdbc/contract-v1.md` 并不存在，最终边界以 `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md` 第 8 节为准。仍需跨入口行为测试、统一 provider factory 所有权、feature 隔离和发布验收。
