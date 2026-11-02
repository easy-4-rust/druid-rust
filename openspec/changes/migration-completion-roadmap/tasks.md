# Tasks

完成项对应的提交、测试与产物索引见 `docs/verification/migration-completion-20261102/task-evidence.tsv`；未执行项保持未勾选。

## 1. P0 基线冻结与修复

- [x] 1.1 记录 Java/Rust SHA、dirty 文件、工具链、feature 与当前 CI 失败清单，生成新的基线说明；验证基线包含工作区哈希且不把历史通过报告当作当前结果。
- [x] 1.2 将生产代码使用的 `memory-stats` 从 druid-admin dev-dependency 调整为正式依赖，或删除该生产调用；验证 `cargo check -p druid-admin --all-features --offline` 通过且 `v1_health` 不再伪造 RSS。
- [x] 1.3 处理 `sql_parse.rs` 未使用 `kw` 与 `shared_row_set/trait_impls.rs` 通配导入；验证相关目标 `cargo clippy --all-targets --all-features -- -D warnings -A clippy::pedantic` 通过。
- [x] 1.4 保留 Filter 类身份的 Java `String.equals` 语义，修正旧 Rust 断言并补充同大小写、不同大小写和首尾空格用例；验证 Filter 回归测试通过且未回退生产实现。
- [x] 1.5 在当前工作区运行默认与 `--all-features` 的 check/test/doc，记录失败与 ignored；验证无未解释失败，ignored 不记为行为通过。证据：Linux CI run `41401827024` job `rust-contracts` success；`cargo test --workspace` 4435 passed / 0 failed / 12 ignored，`--all-features` 4619 passed / 0 failed / 16 ignored；check/clippy/doc 同 job 通过。见 `linux-ci-all-features.md`。
- [x] 1.6 完成 `filter-class-identity-parity` 和 `exception-sorter-property-parity` 的剩余验收与规格同步；验证两个 change 在全部任务有证据后才能 validate/archive。证据：filter change 已于 2026-11-02 归档；exception-sorter 严格 Clippy exit 0、17 Rust tests、576+118 差分 4 passed、`--strict` 通过，归档为 `2026-11-02-exception-sorter-property-parity`。

## 2. SOURCE_PARITY 与测试资产账本

- [x] 2.1 从 Java core 全部测试根生成用例清单，展开参数化、动态、重复和 disabled 用例；验证来源分母不少于当前审计的 4759 个 `@Test` 且每个 concrete case 有稳定 ID。证据：4759 个 `@Test` + 新增 2219 个无 `@Test` 的 `public void test...`（其中 18 个已手工执行并映射），manifest 现为 6978 个唯一 case；`java-test-cases.tsv` 同步。
- [x] 2.2 盘点全部 fixture、脚本、语料、golden 文件与数据资源，记录源路径和 SHA-256；验证资产分母与当前审计的 822 个资源一致或说明新增差异。
- [ ] 2.3 创建 `docs/migration/source-test-parity.json`，为每个来源用例填写 MIRRORED/ADAPTED/SPLIT/MERGED_APPROVED、目标测试、保留标志与证据；验证无 MISSING/BLOCKED/NOT_APPLICABLE 被隐藏。进展：首批 24 个来源方法已映射：18 个 `bvt.pool.vendor` ExceptionSorter（含无 `@Test` 的手工执行方法）→ `exception_sorter_semantics_test`，4 个 `FilterManagerTest` + `ClearFilterTest#test_filters` + `DruidDataSourceFilterTest#test_filter` → FilterManager/DataSource filter parity 测试；24 MATCH / 0 MISMATCH。其余 4759 个 `@Test` 仍 MISSING、2201 个未发现方法显式 BLOCKED。
- [x] 2.4 将来源资产逐字节复制到目标位置，派生文件另存；运行资产 SHA-256 校验，验证全部目标文件与原件一致。
- [x] 2.5 扩展 `crates/druid-test` 的整仓验收命令与聚合产物，输出每个 case 的 MATCH/MISMATCH/NOT_RUN；验证不存在用生产 crate 局部测试替代整仓验收。证据：`scripts/run_workspace_acceptance.sh` 逐包运行 7 个 crate（全部 exit 0），`source_parity_acceptance` 生成 `source-parity-status.tsv` / `.json`：4759 cases、822 assets、MATCH 0 / MISMATCH 0 / NOT_RUN 4759，明确保留未映射状态。

## 3. JDBC 逐方法证据

- [x] 3.1 重新生成并校验 `java8-method-evidence.tsv` 当前基线，保留 2001 个身份的完整分母；验证旧证据按 SHA/基线归档，未评估项不被批量改写为通过。
- [ ] 3.2 完成 Connection、Statement、PreparedStatement、ResultSet 的逐方法映射与差分，覆盖 1-based 索引、NULL/wasNull、错误、关闭与资源清理；验证每个方法有 Java 用例和 MATCH 证据。
- [ ] 3.3 完成 DatabaseMetaData、ParameterMetaData、ResultSetMetaData 的逐方法验证，区分 null/empty/pattern 与不支持错误；验证声明、合同、adapter 和 logical_proxy 分层结果。
- [ ] 3.4 完成事务、批处理、生成键、保存点、取消和关闭的验证，保留 i64 更新计数、异常链与 exactly-once 清理；验证对应 Rust/Java 差分 MATCH。
- [ ] 3.5 完成 LOB、Array、Struct、Ref、SQLXML 的正向/负向能力验证，覆盖流生命周期和 typed error；验证 capability=true 必须有同 adapter 行为证据。
- [ ] 3.6 完成 XA、DataSource、PooledConnection 与外部池入口的证据，区分资源 recover 和 durable coordinator；验证不支持 XA 的后端返回 0A000 而非本地事务替代。
- [ ] 3.7 回填 `java8-method-evidence.tsv` 的 declaration/contract/adapter/logical_proxy/real_db/capability 结果；验证真实数据库证据只来自实际运行，1976 个未评估项按真实进度减少。

## 4. 领域语义差分

- [ ] 4.1 验收 Pool/Filter/Stat 的并发借还、逻辑 close、PSCache、fatal discard、Filter 顺序、异常分类和统计；验证高风险用例 MATCH 且无新增未批准差异。
- [ ] 4.2 按方言执行 SQL parse → format → parameterize → wall 四列 Java 差分，复核 Doris BUCKETS 等特殊输入的错误合同；验证 sqlparser 成功不被误当 Java 保真。
- [ ] 4.3 验收 RDBC 默认 `UnsupportedOperation` 与 `0A000` 分型、错误分类、NULL、批处理与资源生命周期；验证默认不支持行为不会被代理回退成成功。
- [ ] 4.4 验收 HA/Dynamic 的故障转移、恢复、节点选择和 ZooKeeper 宿主行为；验证未配置真实宿主的项目保持 NOT_RUN。
- [ ] 4.5 验收 DBCP/DBCP2/c3p0/Proxool 的自有方法、配置映射与生命周期；验证别名不直接计完成，源测试和实际行为可追踪。
- [ ] 4.6 汇总当前全域 Java golden/live differential，保留原始输出与批准差异清单；验证全部来源用例 MATCH、harness failure=0、not-run=0。

## 5. Admin 活跃规格收口

- [x] 5.1 按 `openspec/changes/admin-live-console/tasks.md` 完成其六项任务及 API/字段/刷新合同；验证本路线不重复创建控制台或第二协议。
- [x] 5.2 验证复合身份、poll/push 部分失败、字段可用性、有界分页、过滤先于分页和刷新最后一次结果胜出；保留新旧客户端兼容测试。
- [x] 5.3 运行 Admin 真实进程及 Mobile/Tablet/Desktop 浏览器验收，覆盖鉴权、空/错/部分数据、详情、复制、慢 SQL 与连接/堆栈；验证无 fixture 回退。
- [x] 5.4 核对 OpenAPI 与 handler 同源，核对采集健康和 unavailableFields；验证未上报字段显示不支持而非默认零。证据：`v1_route_table!` 同时展开 router 与 OpenAPI paths，`route_manifest_paths_are_unique` + `v1_openapi_document_lists_every_served_route` 通过；`health_uses_repository_counters_and_sampling_metadata`、`push_capabilities_keep_real_values_and_mask_unreported_defaults` 覆盖不可用字段。
- [x] 5.5 若 push 需要最大耗时、错误或直方图，另立协议增量 change 扩展 `SqlStatMsg` 及兼容协商；验证不修改本路线的采集热路径且旧客户端可继续读取。评估结论：当前 gRPC push wire 不携带这些字段，`v1_snapshot` 将默认值标为 unavailable，慢 SQL/错误/直方图由 Java 兼容 JSON 来源提供；本路线不扩展协议、不改采集热路径。证据：`admin_live_console_test` 7 passed（含 `push_capabilities_keep_real_values_and_mask_unreported_defaults`），`docs/verification/admin-live-console-20261102.md`「真实缺口与验收边界」。

## 6. ADR-CRATE-003 门面收敛

- [ ] 6.1 解耦 Filter 参数与 `DruidPooledConnection` 等运行时类型，保持现有调用行为；验证原 Pool/Filter 差分 MATCH 且无公共签名意外变化。
- [ ] 6.2 按 P3.3b 移动 Filter/Wall 接口及实现并保留兼容重导出；验证依赖方向仍为 `druid → druid-rdbc`，无 Tokio/Axum/Tonic/具体驱动污染。
- [ ] 6.3 按 P3.4 将 `druid` 收敛为 DataSource、sql::*、builder 和重导出，删除重复实现；验证外部消费者编译、行为差分和 `cargo tree` 均通过。
- [x] 6.4 生成 public-api/SemVer 报告并复核结构债；验证 EX-002/EX-003 文件未增长、例外复审日期保留、没有新增超 800 行 authored 文件。证据：`docs/verification/migration-completion-20261102/structural-and-public-api.md`；30 个 SemVer 类别精确匹配，EX-002 1,128 行、EX-003 1,216 行，无新增超 800 行文件。

## 7. 静态驱动分发

- [ ] 7.1 在 `druid-rdbc::dispatch` 定义 `DriverKind` 与分发模型，保持所有驱动实现同一 RDBC 标准；验证外部消费者不需要修改现有导入或调用。
- [ ] 7.2 实现 `dispatch!` 统一 exec/fetch/metadata 分支，宏只分派不复制执行逻辑；验证不支持操作保留原错误及上下文。
- [ ] 7.3 为 sqlx、toasty、jdbc-agent 三族接入静态路径并运行与动态路径的同一合同测试；验证参数、NULL、顺序、事务和清理行为一致。
- [ ] 7.4 验证非内置驱动的动态注册、协商和生命周期仍工作；增加外部扩展测试，证明无需修改枚举或重编译产品。
- [ ] 7.5 比较动态/静态路径的吞吐和 p50/p95/p99，记录样本、环境与预算；验证性能未达预算时停止推广并保留动态路径。

## 8. Durable 2PC 协调器

- [x] 8.1 冻结事务/参与者状态、`2pc:tx:{id}` 日志模型、决策顺序、unknown/Mixed 和恢复矩阵；验证 Mixed 明确禁止再次 commit 探测。证据：`druid_rdbc::governance::two_phase` 模型 + `two_phase_recovery_model_test` 8 passed + `docs/migration/two-phase-recovery-model.md`；`RecoveryAction::allows_commit_retry` 仅对已落盘 commit 的幂等重放为真。
- [ ] 8.2 实现 SQLite 持久化日志与 WAL/同步语义，保存 schema version、投票、决策和完成结果；验证重启后日志可读且决策顺序不可重排。
- [ ] 8.3 实现协调器 prepare/commit/rollback 的持久化边界，外部副作用前记录必要决策；验证 prepare 后决策前崩溃不会误报终态。
- [ ] 8.4 实现恢复重放与幂等键，跳过已完成外部效果；验证同一日志重复 replay 不产生重复提交、回滚或不可逆命令。
- [ ] 8.5 实现逐参与者 prepared/committed/aborted/unknown/Mixed 结果及只读核验/批准裁决；验证网络错误不视为未提交，Mixed 请求再次 commit 被拒绝。
- [ ] 8.6 在 SQLite/PostgreSQL 上执行 prepare 前、prepare 后、决策前、部分 commit、恢复中和重复恢复的故障注入；验证真实终态、清理和无重复副作用。
- [ ] 8.7 编写 XA 与 durable coordinator 的边界文档及外部调用示例；验证仅 XA 资源合同不再被描述为 durable 2PC，文档示例由公开 API 编译测试覆盖。

## 9. 真实驱动证据矩阵

- [x] 9.1 生成 80 个 profile 的支持基线，记录 declared/experimental/verified/certified、runtimeMode、provider 和平台；验证公开支持数量只统计 verified/certified。
- [x] 9.2 建立统一矩阵 runner，覆盖连接、查询、更新、预编译、元数据、类型、NULL、批处理、事务、保存点、取消、关闭和异常；验证每个结果含产品/版本/feature/平台。证据：`driver_matrix` 输出 `driver-matrix-results.tsv`；SQLite/sqlx 13/13 PASS，其余 79 profiles 1027 NOT_RUN 并保留原因。
- [ ] 9.3 对 SQLx 支持的 MySQL/PostgreSQL/SQLite 运行真实数据库合同与负能力测试；验证未配置环境写 NOT_RUN，ignored 不晋升支持状态。
- [ ] 9.4 对 Toasty、RBDC、DuckDB、libSQL、HTTP SQL 运行其真实适配合同，区分原生与协议兼容；验证不支持能力返回精确错误。
- [ ] 9.5 对 JDBC Agent 支持的后端运行真实 XA session、recover 和数据库终态测试；验证跨 session 恢复不被忽略或仅以 mock 代替。
- [ ] 9.6 对 bb8/deadpool 等外部池运行统一连接、关闭、错误和生命周期合同；验证外部池自有行为与 canonical RDBC 行为分别记录。
- [ ] 9.7 执行断连、数据库重启、超时、致命异常和资源清理矩阵；验证恢复证据与实际错误分类一致。
- [ ] 9.8 用可复核产物晋升 profile 并更新矩阵报告；验证一个产品或单平台通过不外推全部产品/平台，未验证项仍保持原状态。

## 10. Metrics 与 Admin 生产验证

- [ ] 10.1 测量采集对 SQL p50/p95/p99 的影响，验证热路径无 `send().await`、网络或磁盘调用；保留负载、分位数和失败产物。
- [x] 10.2 重复执行 gRPC 断连、重连、重复/乱序 ACK、Admin 重启、饱和停机和未刷出场景；验证 sequence、幂等、full snapshot、未刷出和 shutdown 数值。证据：`grpc-recovery-3x.log` 连续 3 轮 51 passed / 0 failed / 0 ignored；场景映射见 `grpc-recovery-matrix.md`。
- [ ] 10.3 验证 Admin K8s、鉴权、TLS、部署和真实浏览器行为，覆盖部分失败、字段脱敏和只读能力；验证实际运行环境不支持项明确标记。
- [x] 10.4 在 Linux/macOS/Windows 运行 repository 容量和 session churn 内存预算，验证 RSS/working set 可测量、预算未超标且过期身份被清理。证据：CI run `41396400600` 的 `Rust contracts (ubuntu-latest/windows-latest/macos-latest)` 全部 success；三平台 `cargo test --workspace` 覆盖 `repository_churn_memory_budget_test`（128 MiB sampled physical memory）与 `repository_capacity_test`（TTL/身份清理）。

## 11. 最终门禁与发布判定

- [ ] 11.1 运行 `cargo fmt --all -- --check`、默认/all-features check/test/doc 和严格 Clippy，保存原始日志；验证无未批准警告和失败。
- [x] 11.2 使用 Rust 1.95 运行 MSRV 门禁，验证未通过降低 `rust-version` 或更换不受支持工具链伪造通过。证据：修复本机损坏的 1.95.0 工具链后，`cargo +1.95.0 check --workspace --all-targets --locked` exit 0；日志 `msrv-1.95.log`。
- [x] 11.3 生成 public-api 快照与 SemVer 报告，逐项核对公开变更、外部消费者和精确 allowlist；验证无未批准差异。证据：`structural-and-public-api.md`；30 个 SemVer 类别精确匹配，`verify_task6_external_public_contract.py` PASS。
- [ ] 11.4 在冻结工作区运行全量 `cargo llvm-cov --workspace --all-features`，记录 crate/生产文件/排除项/feature/测试范围；验证报告真实生成，scoped 结果不外推全仓。
- [ ] 11.5 运行完整 Java/Rust 源套件和整仓 differential，验证 SOURCE_PARITY 全映射、TEST_ASSET 全哈希、每 case MATCH、harness failure=0、not-run=0。
- [ ] 11.6 运行 benchmark、供应链和真实数据库矩阵，保留性能、依赖和宿主证据；验证无被忽略的必需场景。
- [ ] 11.7 执行灰度与回滚演练，记录恢复时间、状态兼容和旧版本可用性；验证回滚不会留下双实现或丢失事务日志。
- [ ] 11.8 汇总最终发布报告，只有全部必需门禁有证据时才给 RELEASE_READY；验证任何缺口仍保持 NOT_READY 且不执行 crate publish。

## 12. 规格与证据同步

- [x] 12.1 更新四份迁移主文档和当前验证索引，保留历史快照与原始证据；验证当前事实区无相互矛盾的完成状态和第二套现行文档。证据：`docs/migration/README.md`、`迁移验收规范.md`、`迁移路线图.md`、`对象级对照表.md`、`语义迁移对照表.md`、`对象名称一致性检查.md` 均加入 2026-11-02 当前状态块（19/68、NOT_READY），历史段落保留并明确不作当前结论。
- [x] 12.2 将本 change 的任务逐项关联到实际提交、测试和产物，未执行项保持未勾选；验证 `openspec validate migration-completion-roadmap` 通过。证据：`task-evidence.tsv` 覆盖全部 26 个已完成项；`openspec validate migration-completion-roadmap --strict` 通过。
- [ ] 12.3 仅在实现、测试、验证与规格同步全部完成后 sync/archive；验证主 specs 与 change 一致，Admin 和两个语义 change 仍保持各自独立事实源。
