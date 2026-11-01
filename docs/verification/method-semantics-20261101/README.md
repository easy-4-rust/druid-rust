# ExceptionSorter 逐方法语义核验（2026-11-01）

本报告接续 [当前代码核对](../current-code-audit-20261101/README.md)，只更新实际核验范围。**既有分类器和连接处置逻辑继续复用；本批唯一生产修复是 Oracle 自定义错误码的 Java 整数解析语义。不能据此宣布整个 Druid 迁移完成。**

## 基线、范围和事实源

- Brownfield；规格事实源为 OpenSpec `rdbc-jdbc-method-conformance` 及增量 `openspec/changes/exception-sorter-property-parity`，延续既有 Superpowers 文档。没有另起架构迁移，也没有移动 crate。
- Java 当前 HEAD `fa8dc9912637a2f729eef9f55356621fec18d40e`。实时分类 oracle 使用当前工作区的 11 个源码文件，每个原始字节 SHA-256 都匹配既有 `33824c3dec1612711f9bb4e409319bcab2e4cd0e` 清单。因此 harness 输出旧 commit 标签，不代表本次偷偷替换了当前生产源码。
- Rust 起始 HEAD `7b39d0c04d6829493cacb5de125d7e6de50ef5fe`；期间并行 Admin 工作提交到 `468004e1`。本批未创建/切换分支、提交、重置或还原文件。实际构建输入见 [baseline.json](baseline.json)、[end-state.json](end-state.json) 和各 oracle 的源哈希。
- [methods.tsv](methods.tsv) 列出 **12 个 Java 对象、30 个显式声明的方法/构造器/接口签名**：包括接口 2 项、构造器 3 项。隐式构造器、继承展开和整个 DruidDataSource 不计入该分母。每行包含来源位置、源码哈希、Rust 方法、已核验合同、证据和剩余边界。
- 两仓 CodeGraph 已用于定位，再读当前源码和既有测试。分类器实际仍在 `crates/druid/src/core/`，没有按目标架构臆造已迁入 `druid-rdbc`。

## 三类结论

| 分类 | 本批结论 | 处理 |
|---|---|---|
| 已实现但文档滞后 | 九个厂商分类器、Mock 分类、recoverable/SQLState/error-code/message/cause 判定及 fatal 后的池处置已有真实实现 | 复用实现和原测试，未重写分类器；无配置厂商的空 `configFromProperties` 与 Java 源码一致，不能判成 STUB |
| 已实现但缺验证 | 既有 576 条差分未覆盖 BMP Unicode 数字；Oracle 构造器环境配置、默认 Locale、集合别名/null 边界仍缺独立证明 | 增加 118 条配置边界语料及精确集合断言；其余边界继续保留为未闭合 |
| 确实缺失 | Rust `str::parse::<i32>` 拒绝 Java `Integer.parseInt` 接受的全角、阿拉伯等 BMP 十进制数字，导致配置的 fatal code 未生效 | RED 后只修共享解析；37 段数字、ASCII 符号、i32 边界、UTF-16 代理项拒绝等保持 Java JDK 17 规则 |
| 确实缺失，尚未修复 | Java 无效配置项的错误日志，以及 Phoenix 命中时的错误日志，在对应 Rust 方法中没有等价事件 | 日志副作用仍是明确缺口，不归入批准差异，不因返回值一致而宣称方法全语义一致；后续需结合既有 logging 合同补事件与捕获测试 |

主分类域核验了 DB2 的 8 个错误码、Informix 的 17 个错误码、MySQL 的 19 个错误码及 OceanBase 区间/5 层 cause、Oracle 系列的 49 个错误码/TNS 区间/用户错误区间和附加消息。完整顺序及边界见逐方法台账。Java 默认 Locale 大写转换与 Rust Unicode 大写转换没有在所有 locale 下证明等价。

## 测试账本与实际结果

| 账本/证据等级 | 实际执行 | 结果与范围 |
|---|---|---|
| SOURCE_PARITY / V2 | `exception_sorter_semantics_test` 原有 12 项 | 12 passed；保留原测试 ID 和断言，不以新增用例替代 |
| SOURCE_PARITY / V4 | 原有 `exception_sorters.tsv` 576 条，当前 Java 源码实时编译并与 Rust 生产入口比较 | **573 MATCH + 3 APPROVED_STANDARD_DIFFERENCE**；0 新未批准差异；三项分别为 Oracle/OceanBase/Phoenix 的 null message 既有批准偏差 |
| SOURCE_PARITY / V4 + VALUE_ADD | 独立 `exception_sorter_property_cases.tsv` 118 条：37 个 BMP 数字块及 22 类边界 × 两个 Oracle 分类器 | RED：34 MATCH、84 MISMATCH；GREEN：118 MATCH，0 允许差异。原 576 条资产未改 |
| RUST_OBLIGATION / V1 与集合合同 V2 | 新增 `exception_sorter_property_contract_test` 2 项 | RED：2 failed；GREEN：2 passed。验证精确集合、追加/去重/替换、正负及邻接码行为 |
| SOURCE_TEST_EXECUTED | 当前 Java vendor 目录 7 个原测试类、18 个原方法，使用原 PoolTestCase 初始化和清理 | 18 passed；[逐源测试方法](source-test-methods.tsv)。不是把 18 个方法与 576/118 行相加计算覆盖率，也不是宣称逐源断言与 Rust 已一一绑定 |
| SOURCE_PARITY / V4 + 池副作用 | 原冻结 Mock 测试与 Rust 对照；再编译执行当前 Java Mock 测试、调用原 setup/teardown 并采集同一观察字段 | 冻结版本 1 MATCH；当前版本原断言通过，分类器名、异常类型/码/SQLState/消息、discard/active/idle 字段逐字节匹配 Rust。**池本体依赖 Druid 1.2.28 jar**，不是当前 Java 全池源码验收 |
| RUST_OBLIGATION / V1 | `task6_pooled_connection_exception_paths_test_parity` 1 项、`task6_owner_disposition_control_test` 2 项 | 3 passed，含错误路径通知、同步 fatal 信号、owner 单次移交/关闭；不等于真实多数据库故障演练 |

普通 Rust 联合回归为 17 passed；分类差分 harness 为 4 passed；冻结 Mock harness 为 2 passed。覆盖率运行包含前 17 项和分类 harness 4 项，共 **21 passed / 0 failed / 0 ignored**。Mock 的 2 项另行执行，未混入该次覆盖率数字。

### Java 测试基线问题及处理

直接让旧 Mock harness 使用当前 Java 测试文件时，严格 SHA 校验按预期失败（`4153545e...` 对 `cf1665f2...`），证据保留在 `logs/green-differential.log`。当前测试由 JUnit TestCase 改成 Jupiter，不能把这个失败说成 Rust 业务偏差，也没有放宽哈希保护。

1. 在 D 盘独立目录从原 commit 提取冻结文件。Git blob LF SHA 为 `df66b9a96faec30ea9de73ce60d0bab95dbf11f9c6cadeee74b1d944a21d4a27`；重建当初 Windows checkout 的 CRLF 资产后精确匹配 `cf1665f28399e9bb17d9e791651d4f5a3941bfe54c29cc5d93e1a7385cabedfd`。原引用仓库和冻结清单未修改。
2. 另编译**当前未改动** Java 测试和当前分类器源码，用 Jupiter 依赖执行原断言；Mock 原采集器本来就显式调用生命周期，无需重写测试。
3. 当前 7 个 vendor 类不再继承 JUnit TestCase，且 `test_*` 方法没有 `@Test`。本批采用保留在 `reproduce/SourceSorterTests.java` 的显式方法执行器，严格要求 18 个方法，逐方法实例化并执行原 setup/teardown；不把 JUnit 零发现算通过。

[current-java-inputs.json](current-java-inputs.json) 记录所有当前 Java 源码、runner 和依赖 jar 哈希；[current-java-commands.json](current-java-commands.json) 是实际 argv。当前源测试兼容 runner 是审计补证工具；旧 Mock harness 默认指向当前仓库时的哈希冲突仍存在，需后续明确更新基线，不能掩盖。

## cargo-llvm-cov 与门禁

所有编译、oracle class、临时文件和原始 coverage JSON 均在 D 盘。没有生产文件排除正则。

| 文件/范围 | Regions | Functions | Lines |
|---|---:|---:|---:|
| `abstract_oracle_exception_sorter.rs` | 98.73% | 100.00% | 100.00%（44/44） |
| `oracle_exception_sorter.rs` | 89.23% | 100.00% | 90.57%（48/53） |
| `ocean_base_oracle_exception_sorter.rs` | 90.67% | 100.00% | 92.54%（62/67） |
| 其余 9 个所选 sorter/接口文件 | 100.00% | 100.00% | 100.00% |
| 本次所选包报告 TOTAL | 14.76% | 11.99% | **16.13%（4782/29643）** |

Oracle 两构造器未进入“环境配置存在”的 5 行分支；共享解析有 1 个未覆盖 region。**未采集 branch/MCDC coverage**，JSON 的零分母不能解释为 100% 或真实 0% 分支覆盖率。明细见 [sorter-coverage.tsv](sorter-coverage.tsv)。TOTAL 是此次有限集成测试实际纳入文件的分母，不能冒充全工作区覆盖率，也不能和旧全量覆盖率直接比较。

`cargo llvm-cov report` 导出 JSON 时必须保留 `-p druid -p druid-bench` 才能与测试时摘要使用相同分母。曾不带包参数导出更宽范围 JSON（40697 行、12.26%），它不是另一轮测试结果；原文件留在 D 盘，最终采用 `coverage-selected.json`（29643 行）。[coverage-summary.json](coverage-summary.json) 同时记录两者哈希，避免挑选分母造成误报。

- `rustfmt --check` 三个变动 Rust 文件：exit 0。
- `openspec validate exception-sorter-property-parity --strict`：exit 0。
- 严格 Clippy：exit 101，既存 `crates/druid-rdbc/src/extensions/shared_row_set/trait_impls.rs:1` 的 `use super::*` 阻断；没有关闭 lint 或改动该并行范围。后续 lint 可能尚未执行，不能声称零警告。
- 没有进行全 workspace/all-features/MSRV/真实数据库/70+ 产品/2PC 崩溃恢复验收；本批不宣称满足整体迁移技能全部完成门禁。OpenSpec 保持未归档。

## 复现

先在仓库根目录加载 [环境脚本](reproduce/environment.ps1)，该脚本只把临时与构建路径指向 D 盘，Java/JUnit jar 从现有 C 盘缓存只读加载。工具版本和输出在 `logs/`、`oracle/`；原始生成和运行脚本保存在 `reproduce/`，含当时的绝对路径。

```powershell
. D:\codex-build\druid-sorter-methods-20261101\environment.ps1
cargo test --offline --locked -p druid --test exception_sorter_property_contract_test --test exception_sorter_semantics_test --test task6_pooled_connection_exception_paths_test_parity --test task6_owner_disposition_control_test
cargo test --offline --locked -p druid-bench --no-default-features --test java_rust_exception_sorters_differential -- --include-ignored --nocapture
$env:CARGO_LLVM_COV_TARGET_DIR='D:\codex-build\druid-sorter-coverage-20261101'
cargo llvm-cov --offline --locked -p druid -p druid-bench --no-default-features --test exception_sorter_property_contract_test --test exception_sorter_semantics_test --test task6_pooled_connection_exception_paths_test_parity --test task6_owner_disposition_control_test --test java_rust_exception_sorters_differential --summary-only -- --include-ignored --nocapture
cargo llvm-cov report -p druid -p druid-bench --json --output-path D:\codex-build\druid-sorter-methods-20261101\coverage-selected.json
python D:\codex-build\druid-sorter-methods-20261101\run-current-java.py
```

冻结 Mock 复现先运行 `prepare-source-validation.py`，只在该次命令设置 `DRUID_JAVA_ORACLE_ROOT=D:\codex-build\druid-sorter-methods-20261101\frozen-java`，再运行 `cargo test --offline --locked -p druid-bench --no-default-features --test java_rust_mock_sorter_differential -- --include-ignored --nocapture`。之后重新加载环境脚本恢复当前 Java 路径。当前源码测试 runner 需使用原有 1.2.28 jar/Jupiter/PostgreSQL/MySQL 依赖，完整 argv 不作省略。

## 剩余工作与并行保护

1. 验证两个 Oracle 构造器的隔离进程配置分支，并补无效配置/Phoenix 分类日志事件合同和捕获测试；不要以空壳日志或放宽断言填数。
2. 将 Java Set 可变别名/null setter、Properties defaults/非字符串值、默认 Locale 和宿主系统属性差异分别列入适配合同；未批准前不能计为允许差异。
3. 修复源测试运行配置的 JUnit 发现缺口，并明确当前 Mock 原测试 hash 基线；保留旧冻结版本可复现性。
4. 继续按原核对报告对 Pool/Filter/Stat/HA、SQL/Wall、RDBC/XA、Wrapper、Metrics/Admin 分批做逐方法与源测试映射；本批 30 行不能替代全仓方法分母。

起止哈希检查显示：在记录的输入中，只有本批授权的 `abstract_oracle_exception_sorter.rs` 发生内容变化；已记录的 Admin/Cargo.lock 等并行输入内容保持原样，但对应工作已被另一会话提交，HEAD 因而前移。既有 Filter 修复、审计脚本及其他未提交内容均保留。没有回退并行提交，也没有把它们算成本批修复。
