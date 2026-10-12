# P05 驱动解析基线

状态：未收敛。基线为 `bb1bea1`，以下是源码路径，不代表三条入口已具备相同运行语义。

| 入口 | 当前路径 | 工厂来源 |
| --- | --- | --- |
| `DriverManager::get_connection` | 复制进程内显式注册的 `Driver`，组装 `druid_rdbc::driver::DriverRegistry`，调用 `connect` | 调用方注册的 `Driver`；未自动安装产品驱动 |
| `lookup_driver_extension("sqlite")` | Core 的 `inventory` 扩展查找 | Wrapper 的 `ToastyConnectionFactory` 异步构造器 |
| `DruidDatabasePoolBuilder::build` | `DruidDriverRegistry::builtin` → `resolve` → `DruidPool::builder` | `SqlxConnectionFactory`（SQLite 档案的 `Sqlx` 模式） |

同一 SQLite 产品目前有独立的 Toasty 扩展和 SQLx 产品建池工厂；`DruidDriverRegistry::resolve` 直接实例化具体工厂，没有向标准 `DriverRegistry` 查询。`DriverManager` 的连接操作已委托标准 registry，但其显式注册 overlay 保留 Java 兼容语义。

`driver_extension_registry_test::factory_is_async_runtime_safe` 在当前提交的 Windows Tokio 测试中通过；旧计划所述同步 `block_on` 陷阱已不适用。`docs/rdbc/contract-v1.md` 并不存在，最终边界以 `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md` 第 8 节为准。仍需跨入口行为测试、统一 provider factory 所有权、feature 隔离和发布验收。

## 后续收敛记录

已删除 Core 的 `lookup_driver_extension`、三份扩展描述符/工厂模块及 Wrapper 的 inventory 注册。原 SQLite Toasty 扩展不再构成第二条公开建池路径；产品解析与建池现在均走 `DruidDriverRegistry::resolve`，DriverManager 仍保留显式注册的 Java 兼容 overlay。`driver_resolution_path_test` 证明直接解析和建池选中同一 SQLx 工厂并可实际执行 SQLite 查询；DriverManager 回调合同 5/5、产品注册合同 16/16、全 workspace/all-targets/all-features check 与严格 Clippy 均通过。`DruidDriverRegistry::resolve` 尚未把 provider factory 查找委托给 `druid_rdbc::driver::DriverRegistry`，因此 P05 的“统一 provider registry”仍未关闭，公共 API/SemVer 需在本次删除后重新取证。
