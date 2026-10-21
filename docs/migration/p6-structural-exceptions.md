# P6 结构性例外申请（3 个文件）

> 状态：**PROPOSED（待批准）**。在批准前，这 3 个文件在 `对象级对照表` 中仍计为 `FILE_OVER_800`，P6 不得标记完成。
> 基线：druid-rust `main`，2026-10-22；台账 `docs/migration/inventory/redlines.tsv`。

## 背景

迁移技能规定 authored `.rs` 不超过 800 行。批量拆分后剩余 3 个文件无法在不改变公共 API 或 Rust 语言规则的前提下继续拆分，故按技能要求提交“结构性例外”，逐项给出证据、可选方案与风险。

## EX-001 `crates/druid/src/core/druid_pooled_connection.rs`（2,254 行）— **RESOLVED 2026-10-22**

| 项 | 事实 |
|---|---|
| 组成 | 固有方法 ~1,150 行 + `impl druid_rdbc::connection::Connection for DruidPooledConnection` ~930 行 + Drop/Debug |
| 阻断原因 | Rust 允许一个类型对同一 trait 只有一个 `impl` 块；`Connection` 单块已超过 800 行 |
| 已尝试 | 拆固有方法（成功方向）；把 trait impl 拆成两个文件（**不可行**，语言规则） |
| 可选方案 | A. 把长方法体委托给子模块 helper（需改 ~130+ 行方法体，行为不变）；B. 批准例外并记录复审日期；C. 拆 `Connection` trait 为 supertrait（**破坏公共 API**，不建议） |
| 建议 | **A + 短期 B**：先批准例外，再用 helper 下沉把 trait impl 压到 800 行以内 |
| 结果 | **已按 A 解决**：`database_meta_data`、`prepare_statement_with_generated_keys`、`is_valid`、`ping` 方法体下沉到 `standard_connection_helpers.rs`；trait impl **797 行**；固有 impl 拆为 525/547；父文件 169 行。无需例外。 |

## EX-002 `crates/druid-rdbc/src/extensions/row_set.rs`（1,128 行）

| 项 | 事实 |
|---|---|
| 组成 | 单个 `pub trait RowSet: ResultSet`，约 100 个默认方法 |
| 阻断原因 | trait 定义是单一语言项，不能跨文件拆分；拆成多个 trait 会改变下游实现与调用形态 |
| 可选方案 | A. 批准例外；B. 设计 supertrait（`RowSetNavigation`/`RowSetMutation` 等）并做 SemVer 评估；C. 把默认方法体移到自由函数（trait 定义仍在一处，但行数下降） |
| 建议 | **C 优先**：默认方法体委托给同 crate 的 helper 模块，保持 trait 形状不变；若仍超限则 B |
| C 实测（2026-10-22） | 脚本把 68 个默认方法体迁到 `helpers.rs`（653 行），trait 文件仍 **826 行**（文档 204 行 + 签名 + 未迁移方法）；即使全部迁移，签名与文档仍超过 800。结论：**C 不足以关闭 EX-002**，需要 B（例外）或 supertrait（API 变更）。实验已回滚。 |

## EX-003 `crates/druid-rdbc/src/result_set/result_set_ext.rs`（1,216 行）

| 项 | 事实 |
|---|---|
| 组成 | 单个 `pub trait ResultSetExt`，100 个默认方法（类型/标签/时间/LOB 读取扩展） |
| 阻断原因 | 同 EX-002：单 trait 定义不可拆 |
| 可选方案 | 同 EX-002（C 优先） |
| 建议 | 与 EX-002 一并处理，避免两套风格 |
| C 预估 | 84 个默认方法体共 402 行；迁移后 trait 文件约 899 行（文档 116 行 + 84 个签名 + 未迁移部分），**C 同样不足以关闭 EX-003**。 |

## 结论（2026-10-22）

- EX-001：已解决，无需例外。
- EX-002 / EX-003：C 方案实测/预估均无法把文件压到 800 行以内；单 trait 定义的文件体积由“文档 + 签名”决定，而非方法体。
- 因此只剩两条路：**B（批准结构性例外，附复审日期）** 或 **拆 trait（supertrait/API 变更，需 SemVer 评估）**。

## 批准要求

1. 明确选择 A/B/C 中一项（可对三项分别选择）；
2. 若选 B，指定复审日期与责任人；
3. 若选 C，需附 public-api 快照对比与 SemVer 结论；
4. 批准前，路线图 P6 保持 `IN_PROGRESS`，`crates/druid-test` 可以启动，但整仓“结构门禁”不通过。

## 当前进度

- 已完成拆分：**26 / 28**（超 800 行文件从 28 降到 **2**，均为单 trait 定义）；
- 剩余 2 个：EX-002、EX-003（均为单 trait 定义）；
- 所有拆分后 `cargo clippy -D warnings`、`cargo test -p druid --lib`（161）与 wrapper all-features（69）通过。
