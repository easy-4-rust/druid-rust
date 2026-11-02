# Wall 配置与回调内核下沉：执行记录

## 结论与范围

**P3.3a 的 Wall 配置/回调归属迁移已实现；整个 Druid 迁移尚未完成，完整验收门禁未通过。**
Java 基线：fa8dc9912637a2f729eef9f55356621fec18d40e。
Rust 基线：ed161654c203f60e87ea11b4524ca57496a10e73。
实现与验证首先在独立 worktree druid-rust-f395b762 完成；原目录存在另一批并行 admin 修改，整仓测试结果不能代表其合并后的状态。

本批承接 ADR-CRATE-003 P3.3a，将 WallConfig（含 Builder）、TenantCallBack（含 StatementType）、WallUpdateCheckHandler 的唯一实现移到 druid-rdbc::governance::wall。旧 druid::sql 路径为显式重导出；所有现有方法、字段、注释、Arc 共享语义和调用方保留。Value 引用改为内核的 types::Value。20 个 Wall 规则资源由内核拥有，内容与当前 Java、原 Rust 逐字节 SHA-256 一致；旧资源保留以免影响其他 include 路径。

对原实现仅转换类型导入和 include 相对路径，经过相同 rustfmt 规范化后，三个实现与 HEAD 原文件一致。WallConfig 653 行属于单个对象及其 Builder/装载职责，记录 501–800 行内聚复核，不为行数机械拆分。本批未增加 rdbc 对 druid、驱动、metrics、admin 或 Web 框架的反向依赖。

## 已执行与证据

| 项目 | 实际结果 | 证据 |
| --- | --- | --- |
| 冻结源码/资源分母 | 完成；3 个实现、20 个资源 | baseline.json |
| 单一实现、旧路径兼容、资源归属 | 完成；规范化等价、哈希一致、依赖方向通过 | structural-check.json |
| 当前 Java 默认值/资源观测 | 63 个标量默认字段、5 方言 × 10 规则集合；Rust 全部匹配 | java-oracle.json、java.stdout、fixture 中 java-observations.tsv |
| Java 原 WallConfigTest | 当前源码的 8 个 test_* 方法实际运行通过；不等于已完成逐项 Rust 源用例迁移 | java.stderr、原源码 fixture |
| Wall 回归与覆盖率 | 22 个测试目标，416 通过、0 失败、0 忽略 | coverage-command.json、coverage.log、coverage-summary.json |
| 全仓初次回归 | 4,371 通过、40 失败、12 忽略，exit 101 | workspace-test-command.json、workspace-test.log、workspace-test-summary.json |
| 历史 FilterChain 断言修正 | 生产代码不变；修正两份生成测试的共用类名比较断言 | filter-class-identity-baseline.json |
| 修正后定向回归 | 37 个历史 FilterChain 用例 + 6 个新 Wall 用例，43/43 通过 | final-boundary-regression.log |
| 3 个 admin 超时跟进 | 同一构建的三个测试单独运行全部通过；并发不稳定性仍开放 | failure-followup.json、admin-*-isolated.log |
| Rust 1.95 默认编译 | workspace/all-targets check exit 0；存在基线警告 | compile-gates-msrv.json |
| Rust 1.95 全功能编译 | 首次构建因资源压力主动停止；单并发续跑 workspace/all-targets/all-features exit 0，仍有基线警告 | msrv-valid-all-features-check.log、single-job-gates.json |
| Rust 1.95 Clippy | 未运行成功：该工具链缺 cargo-clippy；不是代码通过 | msrv-valid-strict-clippy.log |

Java oracle 使用当前源码编译覆盖被测类型，druid 1.2.28 jar 仅作辅助依赖，JDK 17 与 jar/source 哈希、实际 argv 已记录。此依赖边界不能据此推广为整套 Java 从源码构建通过。原 Java 测试与源文件保存在 druid-test/tests/fixtures/wall_config_kernel/source，未经删改。

## 覆盖率解释

cargo-llvm-cov 对下沉后的 canonical WallConfig 统计：
- 行：415/416，99.7596%。
- 函数：56/56，100%。
- 区域：397/399，99.4987%。
- 实例化：90/101，89.1089%。

同一批测试的 druid + druid-rdbc + druid-test 合计行覆盖率为 8,468/40,697，20.8074%；这既不是全工作区覆盖率，也不满足“整个迁移已达标”的声明。原始 coverage-canonical.json 保留，不使用排除规则缩小分母。branches/MCDC 分母为零表示未测量；纯 trait 没有可执行条目，不报 100%。未覆盖的 split.next 防御返回及资源行分支保持原实现，不为提高数字删除。

分类：
- SOURCE_PARITY：63 个默认字段、50 个规则集合，以及类名精确比较的 4 个 Java 运行观测。
- RUST_OBLIGATION：内核/门面类型身份、回调 Arc 身份、clone 共享、资源归属、公开 API 兼容。
- VALUE_ADD：内核配置进入真实 Wall 路径，拒绝 DELETE、允许 SELECT、拒绝禁用函数；不冒充 Java 原始用例。

## 失败归因与决策记录

1. 按指定迁移技能先冻结批次，再统一验证；没有按对象逐个宣布迁移完成。
2. 本批为归属迁移，未将所有历史 Rust 行为升级为 Java 等价。构造器系统属性、configFromProperties、标识符 normalize 和异常边界需后续逐项闭合。
3. 延续已接受的 governance/wall 路径；与 Java 包路径的偏差可见，重导出不增加迁移完成率。
4. 全仓暴露 37 个历史生成用例共用错误的大小写断言。修正前生产实现及两个测试文件均与 HEAD 相同。当前 Java FilterManager#existsFilter 使用 String.equals；源码编译观测为 exact=true，uppercase/lowercase/trailing_space=false。本批附加修正只更正测试并补反向断言，保留历史 case ID。37 个名字不能据此视为 37 个不同 Java 原始行为逐项等价。
5. 原工作区后来出现并行 admin 前后端改动；回填仅包含本批拥有的文件，不覆盖共享迁移文档或 admin 改动。
6. 多构建并发时 rustc 出现 alloc_error/STATUS_STACK_BUFFER_OVERRUN；保留失败日志，停止原全功能构建后用 CARGO_BUILD_JOBS=1 顺序重跑。初次工具链 1.95.0 标签缺 manifest；有效标签 1.95 实际 rustc 为 1.95.0。均不计为通过。
7. 初次 cargo test 使用无效 --keep-going 参数，保留错误记录后改用 --no-fail-fast；不隐去失败尝试。

独立只读审查：Wall 生产迁移无 Critical/Important 问题；类名断言补充修正无问题，未降低断言。延后一个 Minor：默认字段测试目前固定覆盖 63 项，未来 Java 新增字段时应自动检查 fixture 键是否被全部消费；本轮固定分母已明确，不以未来键覆盖推定当前失败。

## 尚未闭合

- 完整 Java 源用例逐项映射、双语言全部断言与异常差分；9 个需要固定 Java 环境的差分测试仍在全仓运行中被忽略。
- 3 个真实数据库测试需要 PostgreSQL/MySQL 服务；未伪装成通过或连接到未知数据库。
- Filter 上下文及剩余核心下沉、druid 门面收敛；本次不表示 druid-rdbc 已拥有全部核心实现。
- 20+ 异构池枚举/宏分发的执行和元数据矩阵、70+ 数据库实际能力验收。
- 持久化 2PC 协调器、2pc:tx:{id} 崩溃恢复、Mixed 禁止重新 commit 探测的完整实证。XA 接口/单资源 recover 不能替代这些验收。
- 全仓警告、整仓测试稳定性、完整覆盖率、所有默认/全功能门禁。
- admin 静态前端产物在本隔离快照中未构建（embedded 0 console assets 警告）；原目录并行实现另行验收。

当前六个目标 crate 均已有目录，另有 druid-test；目录存在不表示职责迁移和原语义验收已完成。本记录依据实际代码与运行结果，不沿用历史“完成”标签。

## CodeGraph 与回填
两个源仓库均已通过 CodeGraph 检查和聚焦调用关系查询；原 Rust 仓库回填后执行 codegraph sync 并重新 explore WallConfig，索引已显示 crates/druid-rdbc/src/governance/wall/wall_config.rs。codegraph-final.json 及对应 stdout 保留最终状态。transfer-manifest.json 保存 44 个回填文件的前后 SHA-256；回填前逐个验证本批目标未被并行工作修改。没有提交、推送或覆盖并行 admin/共享文档修改。

附加门禁修正：当前稳定工具链严格 Clippy 定位 SharedRowSet 的历史通配导入。仅将 use super::* 展开为实际使用的显式导入，不修改任何方法体、不加 allow；原失败日志保留。

回填后原目录的 43 项定向测试全部通过（post-transfer-gates.json、original-workspace-boundary.log）。随后 SharedRowSet 仅显式化导入，方法体归一换行后与基线逐字节相同，独立审查通过。当前工具链严格 Clippy 随后报告 druid 32 项既有错误；详见 clippy-baseline-errors.json，不以此宣称整个工作区 lint 通过。

最终复核：SharedRowSet 显式导入修正后，druid-rdbc 的 Rust 1.95 all-targets/all-features check 与当前工具链 all-targets Clippy -D warnings 均 exit 0（rdbc-final-gates.json）。44 个回填代码/fixture 文件与验证 worktree 字节一致，rustfmt --check 和 git diff --check 通过（final-validation.json）。这些局部门禁不抵消 druid 的 32 项基线 lint 和全仓尚未完成的验收项。
