# 准备批证据（2026-10-14）

本记录对应任务 1.1–1.5。它记录已验证的基线与下一批执行条件，不代表后续功能已实现。

## 仓库与架构基线

- 仓库：`E:\workspaces\workspace-github-easy-4-rust\druid-rust`；分支 `main`；审查 HEAD 与 `origin/main` 均为 `af9a6946d8b02806f61246e50843656cb0af8730`，远端无领先提交。
- 工作区已有 `README.md`、`README.zh-CN.md` 的标准合同架构图修改，检查后保留；本记录及该图之外无其他用户代码改动。
- CodeGraph 在当前索引确认 `DriverRegistry::connect` 仍先 `accepts_url` 后连接且以 `?` 提前传播错误；`druid-rdbc::XaConnection` 仍仅公开资源名称；产品侧 `CommonDataSource` 还没有中立标准合同。前序审查记录的 Timestamp/Time、PooledConnection 监听器差异与目前分支源码一致。
- 架构依据：[ADR-RDBC-006](../../../../docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md)。前序基线测试 610 passed、纯度检查 119 files passed，是历史值，不抵扣此变更测试。

## Java oracle 与来源

- 冻结参考源码：OpenJDK 8u `jdk8u502-b07`，commit `7373384725418fa645fb4508ec05a869bc7fc3e7`；归档 SHA-256 `5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b`。参考测试源码时记录该身份；本机可运行版本记录为 Oracle JDK/JRE 8u321-b07（Java 与 `javac` 均为 1.8.0_321），不把补丁版本相同视作 8u502 源码运行证据。
- 8u321 的 `America/New_York` tzdb 版本为 `2021e`。用 `-Duser.timezone` 在三个独立 JVM 运行同一 oracle；各进程输出：

| 时区 | epoch 0 的本地字段 | 本地 `1970-01-01 00:00:00` 的毫秒 |
|---|---|---:|
| UTC | `1970-01-01T00:00` | `0` |
| Asia/Shanghai | `1970-01-01T08:00` | `-28800000` |
| America/New_York | `1969-12-31T19:00` | `18000000` |

- 三时区均观察到 Timestamp 秒精度输出 `.0`、1 纳秒输出 `.000000001`、120000000 纳秒输出 `.12`；`Time.valueOf(LocalTime 12:34:56.123456789).toLocalTime()` 得到 `12:34:56`、纳秒为 0。New York 的 gap `2024-03-10 02:30` 读取本地字段时规范化为 `03:30`；overlap `2024-11-03 01:30` 保持该本地字段。毫秒结果详见后续差分测试，不据此假定 Rust 时区算法等价。
- `javap -public` 对 JDK 8u321 的目标类核验结果记录在 [api-gap-map.tsv](api-gap-map.tsv)。该映射用于实施准备；最终 API 核对仍需对冻结 8u502 源码逐项复核。

## 适配器与环境

- SQLx SQLite 是现成取消验收后端：执行 `cargo test -p druid-wrapper --features 'sqlx,jdbc-agent' --locked --offline --test sqlx_statement_cancel_contract_test --test task6_jdbc_agent_h2_contract_test_parity -- --nocapture` 时，取消合同 **4 passed**。此命令中的 H2 用例首次运行因 PATH 指向 Java 8 而失败（当前 Agent jar class version 61）；将该子进程 PATH 前置到 `D:\Java\jdk-17.0.12\bin` 后，使用 SQLx SQLite 单独用例通过。
- JDBC Agent + H2 2.3.232 是 XA 后续候选：本地已存在 Agent/H2 jar，使用 Java 17 执行现有合同命令 `cargo test -p druid-wrapper --features 'jdbc-agent' --locked --offline --test task6_jdbc_agent_h2_contract_test_parity -- --nocapture`，结果 **1 passed**，证明 Agent 与内存 H2 实际连接可用。该测试没有验证 XA，XA prepare/recover 闭环仍是后续验收项。
- Docker 另有 `postgres:18.6` 容器在运行且通过 `pg_isready`；容器自启以来的数据库由其他测试/任务管理，本变更不连接或改写其中的数据。取消先选 SQLite，XA 先选已验证的 H2 Agent；若真实 XA 能力不满足，保留阻断证据并重新核定准备条件，不把连接成功视作 XA 支持。
- 可复现的定向命令、首轮 JDK 选择失败和修正均如上。Cargo 本地命令期限暂定每个定向批 30 分钟；依赖 Java Agent 的批在进程环境中显式设置 `DRUID_JDBC_AGENT_JAR`、`DRUID_JDBC_DRIVER_JAR` 并使用 Java 17。超过期限停止记录，不自动续跑。

## 分批命令与证据位置

预期输出均写进本 OpenSpec 变更的 `evidence/`，测试源码放入对应 crate；命令是计划，只有实际退出码 0 才登记通过。

| 批次 | 主要定向命令 |
|---|---|
| 驱动协商 | `cargo test -p druid-rdbc --test driver_registry_negotiation_contract_test --locked --offline`；`cargo test -p druid --test driver_manager_negotiation_contract_test --locked --offline` |
| 时间值 | `cargo test -p druid-rdbc --test jdbc_temporal_value_contract_test --locked --offline`；Java 8 oracle 子进程按 UTC、Asia/Shanghai、America/New_York 分开运行 |
| 取消 | `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`；定向 SQLite 慢执行与重用案例 |
| 池事件 | `cargo test -p druid-rdbc --test pooled_connection_listener_contract_test --locked --offline`；`cargo test -p druid --test pooled_connection_event_contract_test --locked --offline` |
| XA | `cargo test -p druid-rdbc --test xa_resource_contract_test --locked --offline`；配置 Java 17 Agent/H2 后运行 `task6_jdbc_agent_h2_contract_test_parity` 中新增 XA 恢复案例 |
| 公共数据源 | `cargo test -p druid-rdbc --test common_data_source_contract_test --locked --offline`；`cargo test -p druid --test common_data_source_contract_test --locked --offline` |
| 方法证据 | 对应方法清单生成器的 fixture 测试及 `python scripts/verify_rdbc_single_standard_purity.py --phase final` |
| 最终门禁 | `cargo test -p druid-rdbc --locked --offline --quiet`；`python scripts/verify_rdbc_single_standard_purity.py --phase final`；相关 workspace feature 构建一次 |

如果现存消费者已有覆盖，实施时更新原测试而非并行创建重复文件；本表的命名为准备阶段的目标入口。每个批次只重跑被修改影响的命令，最终综合门禁只运行一次。
