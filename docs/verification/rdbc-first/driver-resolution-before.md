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

## 2026-10-12 工厂入口收敛

标准 `DriverRegistry` 现持有具名 `ConnectionFactoryProvider`，重复安装同名 provider 时以最后一次为准。Wrapper 在装载目录时安装启用 feature 对应的 SQLx、DuckDB、libSQL 和两个 HTTP SQL provider；`with_jdbc_agent` 显式安装 JDBC Agent provider。`DruidDriverRegistry::resolve` 保留产品档案、URL 和 feature 校验，再通过标准注册表查询 provider 并取得工厂，建池入口仍复用该解析结果。`DriverManager` 继续使用标准注册表的显式 `Driver` overlay。

验证：具名 provider RED 测试先因缺少注册/查询 API 编译失败，新增接口后 1/1 通过；SQLite 直接连接与建池 1/1、SQLx 产品档案 16/16、全 feature 产品档案 17/17、全 feature SQLite/原生/HTTP 解析 2/2、无 SQLx 的目录隔离 1/1，以及无默认 feature/全 feature 编译均通过。此记录仅关闭 P05 的产品工厂查找缺口；P05 整体还需当前提交的公共 API、SemVer、CI 和最终范围验收。

补充验证：`verify_wrapper_standard_impls.py` 在 D 盘 Cargo/临时目录下通过，报告 31 个活跃标准适配器、21 个已闭合消费者、7 个运行跟踪家族及 7 个编译覆盖家族；`verify_feature_dependency_boundaries.py` 与 `verify_package_ownership.py` 通过；`cargo check --workspace --all-targets --locked` 通过。当前 SemVer 报告把默认 feature 缩减识别为 `druid-wrapper/feature_not_enabled_by_default`，已以真实指纹登记为 pending；连同此前 10 类 pending，共 11 类尚待迁移和外部消费者审核，不能据此宣布 P05 完成。
