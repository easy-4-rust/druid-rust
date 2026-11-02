# migration-completion-roadmap 执行进展

更新：2026-11-02

## 已完成（26/68）

| 任务 | 证据 |
|---|---|
| 1.1 基线冻结 | `baseline.md`、`baseline-dirty-paths.txt`（SHA-256 `e3f0068a…`） |
| 1.2 `memory-stats` 正式依赖 | `crates/druid-admin/Cargo.toml` 正式依赖段；`cargo check -p druid-admin --all-features` 通过 |
| 1.3 严格 Clippy | 修复 `sql_parse.rs` 未使用 `kw`、Admin 12 个 cast/doc 错误；`cargo clippy -p druid-admin -p druid-rdbc --all-targets --all-features -- -D warnings -A clippy::pedantic` exit 0 |
| 1.4 Filter 回归 | part_01 34 + manager 3 + part_02 6 = **43 passed**；保留 Java 大小写敏感语义 |
| 1.5 全量默认/all-features | Linux CI run `41401827024` job success：默认 4435 passed/0 failed/12 ignored；all-features 4619 passed/0 failed/16 ignored；check/clippy/doc 通过。见 `linux-ci-all-features.md` |
| 1.6 前置 change 收口 | `exception-sorter-property-parity`：严格 Clippy exit 0、17 Rust tests、576+118 差分 4 passed、`--strict` 通过，归档为 `2026-11-02-exception-sorter-property-parity`；filter change 已归档 |
| 2.1 Java 用例清单 | `java-test-cases.tsv`：4759 个 `@Test` + 2219 个无 `@Test` 的 `public void test...`，manifest 共 **6978** 个唯一 case |
| 2.2 资源清单 | `java-test-assets.tsv`：**822** 个资源、13.46 MiB |
| 2.4 资源精确复制 | 复制到 `crates/druid-test/tests/fixtures/source-java/src/test/resources/`，SHA-256 **822/822** 一致 |
| 2.5 整仓验收聚合 | `scripts/run_workspace_acceptance.sh` 逐包跑 7 个 crate 全部 exit 0；`source_parity_acceptance` 输出 `source-parity-status.tsv`：6978 cases、MATCH 111 / APPROVED_DIFFERENCE 13 / MISMATCH 19 / NOT_RUN 6835，未隐藏 MISSING/BLOCKED |
| 3.1 JDBC 方法基线 | `java8-method-evidence-baseline.tsv`，2001 身份，SHA-256 `cfa065c8…`，未评估状态不变 |
| 5.2 Admin 数据合同 | Rust 合同 34 passed（`admin-console-contracts.log`）；前端 5 passed（`admin-web-tests.log`） |
| 5.4 OpenAPI 同源 | `v1_route_table!` 同时驱动 router 与 OpenAPI paths；`route_manifest_paths_are_unique` 与 `v1_openapi_document_lists_every_served_route` 通过；健康/unavailableFields 测试覆盖 |
| 6.4 结构债与 public-api/SemVer | `structural-and-public-api.md`：30 个 SemVer 类别精确匹配，EX-002 1,128 行、EX-003 1,216 行未增长，无新增 >800 行文件 |
| 8.1 2PC 恢复模型冻结 | `druid_rdbc::governance::two_phase` + `two_phase_recovery_model_test` 8 passed；Mixed 永不返回 commit 重放，Unknown 只读核验；见 `docs/migration/two-phase-recovery-model.md` |
| 9.1 驱动支持基线 | `driver-profiles-baseline.tsv`：80 profiles，declared 46 / experimental 34 / verified+certified **0** |
| 9.2 统一驱动矩阵 runner | `driver_matrix` 输出 80 profiles × 13 操作；SQLite/sqlx 13/13 PASS，其余 1027 NOT_RUN；见 `driver-matrix-results.tsv` |
| 10.2 gRPC 恢复矩阵 | `grpc-recovery-3x.log` 连续 3 轮 51 passed / 0 failed；断连/重连、重复/乱序 ACK、Admin 重启、饱和停机与未刷出映射见 `grpc-recovery-matrix.md` |
| 10.4 三平台内存预算 | CI run `41396400600` 的 Ubuntu/Windows/macOS Rust contracts 全 success；`repository_churn_memory_budget_test` 与 `repository_capacity_test` 覆盖 128 MiB 预算、RSS 可测量与 TTL/身份清理 |
| 11.2 MSRV 1.95 | 修复本机损坏工具链后 `cargo +1.95.0 check --workspace --all-targets --locked` exit 0，`msrv-1.95.log` |
| 11.3 public-api/SemVer 外部消费者 | 同 6.4；`verify_task6_external_public_contract.py` PASS |
| 12.1 迁移文档当前事实同步 | README + 五份主文档加入 2026-11-02 状态块（26/68、NOT_READY），历史快照保留 |
| 12.2 任务证据关联 | `task-evidence.tsv` 关联 22 个已完成项到提交/测试/产物；`openspec validate migration-completion-roadmap --strict` 通过 |
| 5.1 Admin 活跃 change | `admin-live-console` 六项任务已勾选、`--strict` 校验通过 |
| 5.3 Admin 三尺寸浏览器 | `browser/`：Mobile 390×884、Tablet 768×1024、Desktop 1280×1024 全路由截图；Basic 登录、空态、部分数据、详情、真实点击复制、慢 SQL、连接/stack 能力态；修复登录跳旧页与 SQL/直方图误标记未上报 |
| 5.5 push 协议边界 | 评估后不扩展 `SqlStatMsg`：gRPC push 未携带的最大耗时/错误/直方图由 `v1_snapshot` 标为 unavailable，慢 SQL 走 Java 兼容 JSON 来源；采集热路径未改。证据：`admin_live_console_test` 7 passed（含 `push_capabilities_keep_real_values_and_mask_unreported_defaults`） |

本轮另完成 Java 1.2.28 oracle 恢复：`D:/druid-oracle/druid-1.2.28`，JDK 17 + Maven Central 独立 settings；`cargo test -p druid-bench --features jdbc-agent --tests -- --include-ignored` 的 **11 个 differential 目标全部通过、0 failed**，覆盖 Callable、ExceptionSorter、JDBC42、MockSorter、Pool、PreparedCache、ResultSet、Statement、Wall，日志 `java-rust-differential.log`。

另：`filter-class-identity-parity` 已通过严格 Clippy 和 `--strict` 校验并归档为 `2026-11-02-filter-class-identity-parity`。

## 当前阻塞

### 1. 全量测试：Linux CI 已完成（1.5 已关闭）

- Linux CI run `41401827024` job `rust-contracts` success：默认 `cargo test --workspace` **4435 passed / 0 failed / 12 ignored**；`--all-features` **4619 passed / 0 failed / 16 ignored**；check/clippy/doc 同 job 通过。证据 `linux-ci-all-features.md` / `.log`。
- 本地 Windows all-features gRPC 的 `10060` 回环异常保留为环境说明，不再阻塞 1.5。

### 2. SOURCE_PARITY 首批映射已完成，余量仍大

- `docs/migration/source-test-parity.json` 现为 **6978** 个唯一 case（4759 个 `@Test` + 2219 个无 `@Test` 的 `public void test...`），822 个资产。
- 首批 **143** 个来源方法已映射：18 个 `bvt.pool.vendor` ExceptionSorter 方法 → `exception_sorter_semantics_test`；4 个 `FilterManagerTest` 方法 + `ClearFilterTest#test_filters` + `DruidDataSourceFilterTest#test_filter` → FilterManager/DataSource filter parity 测试。**111 MATCH / 13 APPROVED_DIFFERENCE / 19 MISMATCH**；2201 个未发现方法显式标为 `BLOCKED`；Pool 差分 6 例（3 MATCH + 3 approved）、Callable 7 例（2 MATCH + 5 approved）、ResultSet 5 例（3 MATCH + 2 approved）、Statement 37 例（35 MATCH + 2 approved）、MockSorter 1 例 MATCH、PreparedCache 1 例 approved、WallMultiStatement 2 例 MATCH、WallFilterTest H2 1 例 MATCH、StatementTest#test_stmt 1 例 MATCH；WallUtils 方言测试作为 WallMultiStatement 的第二目标、DruidDataSourceTest_maxWait、notEmptyWait2#test_maxWaitThread、phyMaxUseCount 各 1 例 MATCH；Wall allow 开关 19 例（delete/update/insert/truncate/drop/select-into 16 MATCH，JDBC call escape 3 MISMATCH）；Wall always-true 6 例（select/delete where 4 MATCH，HAVING 2 MISMATCH）；WallUnionTest 2 例 MISMATCH（selectUnionCheck 未拦截普通 UNION）；WallSelectIntoTest1/WallSelectWhereTest0/WallDropTest1 6 例 MATCH，WallDropTest2 DROP PROCEDURE 2 例 MISMATCH；WallSelectWhereTest1/2/3 8 例（6 MATCH + 2 MISMATCH）；WallDeleteWhereTest1 4 例（2 MATCH + 2 MISMATCH）、WallHavingTest6 2 例（1 MATCH + 1 MISMATCH）；WallCommentTest 3 例 MISMATCH（checkValid 一致，但缺 commentDenyStat）；WallDeleteWhereTest2 2 例（1 MATCH + 1 MISMATCH，带注释的 DELETE OR always-true）。另记录 `DruidLoaderUtilsTest#testLoadFilter` 的 log4j/encoding 别名差异为 1 个 MISMATCH，未隐藏。
- 其余 4759 个 `@Test` 仍为 `MISSING/NOT_RUN`；禁止按测试名自动映射或伪造 MATCH。

### 3. JDBC 方法证据：首批 Statement 已回填

- 2001 个身份中 linked 81、unassessed 1920；新增 30 个 `java.sql.Statement` 方法 + `CallableStatement#wasNull()` + `ResultSet` 的 `previous/getRow/getFetchSize/wasNull` + 22 个 PreparedStatement 属性/生命周期方法，声明/合同层 PASS，capability PARTIAL。
- `verify_java8_method_evidence.py` 通过；real_db 仍 0/2001，adapter/logical_proxy 仍未评估，3.2–3.7 未完成。

### 4. 其他未完成阶段

- 门面收敛 6.1–6.3；静态分发 7.x；durable 2PC 8.x；真实驱动 9.2–9.8；Metrics/Admin 生产验证 10.x；最终发布 11.1/11.4–11.8；规格同步 12.x。
- 已完成：1.5、1.6、2.1、2.2、2.4、2.5、5.1–5.5、6.4、8.1、9.1、9.2、10.2、10.4、11.2、11.3、12.1、12.2（详见上表）。

## 下一步

1. 继续 2.3：按 JDBC 核心对象、Pool/Filter、SQL/Wall 分批，只映射已有真实测试与可复核断言，禁止名称猜测；当前 111 MATCH / 13 APPROVED_DIFFERENCE / 19 MISMATCH。
2. 3.2 从 Connection/Statement/PreparedStatement/ResultSet 开始，逐方法关联 Java 用例和 MATCH。
3. 进行 6.1 门面收敛和 7.1–7.2 静态分发的设计/实现；保持兼容重导出和动态路径。
4. 9.3–9.8 在真实 MySQL/PostgreSQL/外部池/故障环境运行矩阵；未配置环境保持 NOT_RUN。
5. 12.3 仅在全部实现、测试、验证与规格同步完成后 sync/archive。
