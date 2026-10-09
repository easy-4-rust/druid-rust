# 三个标准值类型的真实归属与兼容性恢复（2026-10-08）

结论：JdbcType、RowId、PseudoColumnUsage 的真实定义已归属 RDBC 标准模块；旧 RDBC 与 Druid 路径重导出同一类型。Task6 FIX_REQUIRED、Task8 INCOMPLETE，18 项顶层任务继续开放。

## 实现范围

- JdbcType 的真实枚举位于 types/jdbc_type.rs，保留 39 个 Java 8 类型的名称、java.sql vendor 和编号，新增 value_of 编号查找；未知编号返回 InvalidArgument。RdbcType 是同一类型的历史重导出。
- RowId 的真实定义位于 types/row_id.rs，保留原字节、相等、哈希与 get_bytes 副本策略；RdbcRowId 与 Druid RowId 重导出同一对象。
- PseudoColumnUsage 的真实四状态定义位于 metadata/pseudo_column_usage.rs，Druid 历史模块仅重导出。
- mod.rs 只做声明与重导出；没有在 facade 创建第二个对象或空实现。

## 真实 RED、回归与修复

原 Cargo 合同测试在生产迁移前实际 exit101/E0432，三个 canonical 名称缺失。迁移后合同测试实际 3/0，workspace fmt exit0，冻结 standard-value-inputs.json 为 1101 源码及 58 控制输入。

首次完整 RDBC 回归实际 exit0、477/0；随后 Druid lib 实际 exit101、148/2。两项失败均保留原资源转发轨迹断言，显示 RowId 的派生 Debug 从 RdbcRowId 改为 RowId。该差异确为迁移引入，不归因为网络或旧环境。

修复采用 RowId 的真实 Debug 实现，保留原对象名称和字节格式；没有修改原断言或更换对象身份。补充标准/历史/产品路径三者相同 Debug 表示的合同。修复后冻结 standard-value-debug-inputs.json，仍为 1101 源码及 58 控制输入。

| 验证 | 实际结果与边界 |
| --- | --- |
| workspace fmt | exit0 |
| Druid all-features lib | exit0，150/0，包含原两项回归 |
| 标准值归属合同 | exit0，4/0，39 映射与查找、4 未知拒绝、RowId 字节/身份/Debug、4 伪列状态 |
| 完整 RDBC all-features | exit0，477/0，18 结果块，含外部消费者及 doctest |
| final source purity | exit0，STANDARD_FILES=51；仅认证脚本覆盖的源码范围 |
| 严格全 workspace Clippy | 原句柄 3768 实际 exit0，all-targets/all-features/locked/offline/-D warnings，4m32s |
| 原 68 外部类型探针 | 当前 Cargo JSON 返回的 rlib 支持全量复编译，45PASS/23FAIL，另1项非Rust目标未认证；原探针 SHA 均未变化 |
| 当前 JdbcType 与固定 Java 值合同差分 | Rust/Java 编译或运行均 exit0，39 TYPE 输出逐项相同，39 查找成功，4 未知编号均按平台错误策略拒绝；不是完整 differential |

[首次回归及真实失败](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-ownership-regression1.summary.json)

[修复后回归](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-ownership-regression2.summary.json)

[源码纯净性](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-final-purity1.summary.json)

[全量原探针复验](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes5.summary.json)

[当前类型值差分](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-type-value-runtime2.summary.json)

探针第一次运行 probes4 因工具参数将依赖搜索目录设为 debug 而非 debug/deps，68 项均报 E0463 缺 chrono；该工具错误的原日志保留，不作为生产公开路径诊断。修正依赖目录后 probes5 实际得到 45/23/1，未改变生产源码或原探针。Cargo artifact 路径来自明确的 compiler-artifact JSON，未按修改时间选择。

## 尚未认证

固定 OpenJDK 源码及 Java 值合同证据见 canonical residual source preflight 报告；RowId 的来源表、数据源、生命周期、驱动特定 toString 不由本地字节合同认证。

未修改 oracle、语义 inventory 或原 68 探针，未手写 LIVE 晋升。最终 API/SemVer、完整 Java/Rust differential、真实数据库、final CLI 及 Task7 八项仍开放。此前全 workspace 3869/43 对应迁移前 1099 源码冻结；当前尚无完整全 workspace runtime 通过证据。
