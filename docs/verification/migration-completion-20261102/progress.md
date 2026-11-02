# migration-completion-roadmap 执行进展

更新：2026-11-02

## 已完成（20/68）

| 任务 | 证据 |
|---|---|
| 1.1 基线冻结 | `baseline.md`、`baseline-dirty-paths.txt`（SHA-256 `e3f0068a…`） |
| 1.2 `memory-stats` 正式依赖 | `crates/druid-admin/Cargo.toml` 正式依赖段；`cargo check -p druid-admin --all-features` 通过 |
| 1.3 严格 Clippy | 修复 `sql_parse.rs` 未使用 `kw`、Admin 12 个 cast/doc 错误；`cargo clippy -p druid-admin -p druid-rdbc --all-targets --all-features -- -D warnings -A clippy::pedantic` exit 0 |
| 1.4 Filter 回归 | part_01 34 + manager 3 + part_02 6 = **43 passed**；保留 Java 大小写敏感语义 |
| 1.6 前置 change 收口 | `exception-sorter-property-parity`：严格 Clippy exit 0、17 Rust tests、576+118 差分 4 passed、`--strict` 通过，归档为 `2026-11-02-exception-sorter-property-parity`；filter change 已归档 |
| 2.1 Java 用例清单 | `java-test-cases.tsv`：**4759** 个唯一 case ID |
| 2.2 资源清单 | `java-test-assets.tsv`：**822** 个资源、13.46 MiB |
| 2.4 资源精确复制 | 复制到 `crates/druid-test/tests/fixtures/source-java/src/test/resources/`，SHA-256 **822/822** 一致 |
| 2.5 整仓验收聚合 | `scripts/run_workspace_acceptance.sh` 逐包跑 7 个 crate 全部 exit 0；`source_parity_acceptance` 输出 `source-parity-status.tsv`：4759 cases、MATCH 0 / MISMATCH 0 / NOT_RUN 4759，未隐藏 MISSING |
| 3.1 JDBC 方法基线 | `java8-method-evidence-baseline.tsv`，2001 身份，SHA-256 `cfa065c8…`，未评估状态不变 |
| 5.2 Admin 数据合同 | Rust 合同 34 passed（`admin-console-contracts.log`）；前端 5 passed（`admin-web-tests.log`） |
| 5.4 OpenAPI 同源 | `v1_route_table!` 同时驱动 router 与 OpenAPI paths；`route_manifest_paths_are_unique` 与 `v1_openapi_document_lists_every_served_route` 通过；健康/unavailableFields 测试覆盖 |
| 6.4 结构债与 public-api/SemVer | `structural-and-public-api.md`：30 个 SemVer 类别精确匹配，EX-002 1,128 行、EX-003 1,216 行未增长，无新增 >800 行文件 |
| 9.1 驱动支持基线 | `driver-profiles-baseline.tsv`：80 profiles，declared 46 / experimental 34 / verified+certified **0** |
| 11.2 MSRV 1.95 | 修复本机损坏工具链后 `cargo +1.95.0 check --workspace --all-targets --locked` exit 0，`msrv-1.95.log` |
| 11.3 public-api/SemVer 外部消费者 | 同 6.4；`verify_task6_external_public_contract.py` PASS |
| 12.1 迁移文档当前事实同步 | README + 五份主文档加入 2026-11-02 状态块（20/68、NOT_READY），历史快照保留 |
| 5.1 Admin 活跃 change | `admin-live-console` 六项任务已勾选、`--strict` 校验通过 |
| 5.3 Admin 三尺寸浏览器 | `browser/`：Mobile 390×884、Tablet 768×1024、Desktop 1280×1024 全路由截图；Basic 登录、空态、部分数据、详情、真实点击复制、慢 SQL、连接/stack 能力态；修复登录跳旧页与 SQL/直方图误标记未上报 |
| 5.5 push 协议边界 | 评估后不扩展 `SqlStatMsg`：gRPC push 未携带的最大耗时/错误/直方图由 `v1_snapshot` 标为 unavailable，慢 SQL 走 Java 兼容 JSON 来源；采集热路径未改。证据：`admin_live_console_test` 7 passed（含 `push_capabilities_keep_real_values_and_mask_unreported_defaults`） |

本轮另完成 Java 1.2.28 oracle 恢复：`D:/druid-oracle/druid-1.2.28`，JDK 17 + Maven Central 独立 settings；`cargo test -p druid-bench --features jdbc-agent --tests -- --include-ignored` 的 **11 个 differential 目标全部通过、0 failed**，覆盖 Callable、ExceptionSorter、JDBC42、MockSorter、Pool、PreparedCache、ResultSet、Statement、Wall，日志 `java-rust-differential.log`。

另：`filter-class-identity-parity` 已通过严格 Clippy 和 `--strict` 校验并归档为 `2026-11-02-filter-class-identity-parity`。

## 当前阻塞

### 1. 全量测试：默认 7/7 包通过；全 feature 仅 gRPC 目标受 Windows 回环阻塞

- 默认：`cargo check --workspace`、`cargo doc --workspace --all-features --no-deps` exit 0；`druid`、`druid-rdbc`、`druid-admin`、`druid-wrapper`、`druid-metrics`、`druid-bench`、`druid-test` 逐包 `cargo test` 全部 exit 0（`workspace-test-per-package*.log`），含 `grpc_network_e2e_test` 51/51。
- 全 feature：逐包测试除 `druid-admin` 的 gRPC 目标外全部 exit 0；该目标在启用全套特性后出现 `10060` 回环连接超时，而同一目标默认特性 51/51 通过。已把测试等待上限提高到 30s 并用进程内串行门降低并发，仍未消除该环境差异。
- 因此 1.5 保持未完成；已记录这是 Windows 回环/特性组合问题，不是产品断言失败，但不用“其余都绿”替代完整门禁。

### 2. SOURCE_PARITY 有效映射尚未完成

- `docs/migration/source-test-parity.json` 已生成 4759 个用例和 822 个资产记录。
- 当前所有用例仍为 `MISSING/NOT_RUN`；审计确认资产 822/822 精确，但语义映射、目标测试和差分 MATCH 尚缺。
- 禁止按测试名自动映射或伪造 MATCH。

### 3. JDBC 方法证据仍以未评估为主

- 2001 个身份中 capability 仅 25 PARTIAL，real_db 0/2001。
- 3.2–3.7 尚未开始实际方法映射与差分。

### 4. 其他未完成阶段

- 门面收敛 6.1–6.3；静态分发 7.x；durable 2PC 8.x；真实驱动 9.2–9.8；Metrics/Admin 生产验证 10.x；最终发布 11.1/11.4–11.8；规格同步 12.x。
- 已完成：1.6、5.4、5.5、6.4、11.2、11.3；5.1–5.3 见上表。

## 下一步

1. 在稳定 Linux CI 或非受限 Windows 环境重跑 `--all-features` gRPC 目标，关闭 1.5。
2. 开始 SOURCE_PARITY 语义映射：按 JDBC 核心对象、Pool/Filter、SQL/Wall 分批，先映射现有真实测试，禁止名称猜测。
3. 3.2 从 Connection/Statement/PreparedStatement/ResultSet 开始，逐方法关联 Java 用例和 MATCH。
4. 进行 6.1 门面收敛和 7.1–7.2 静态分发的设计/实现；保持兼容重导出和动态路径。
5. 12.1 在最终事实稳定后统一四份迁移主文档，避免中途反复改写。
