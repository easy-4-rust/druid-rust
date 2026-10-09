# Task6 缓冲 LOB 列名回归恢复（2026-10-06）

完整 workspace 的原会话41767句柄已缺失，cargo51544已不存在；日志包含完整文档测试及最终失败目标，确认终态失败。242结果块：3705 PASS / 3 FAIL / 0 ignored，包含2个文档测试。失败为两个 Callable prepared5vs6 原案例，以及 `buffered_resource_null_updates_preserve_state_and_stream_refusal_is_non_consuming`。日志：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-filter-update-workspace.log`。没有重启该全量命令；工具句柄丢失不能单独证明终态，此处还核对了实际进程及完整日志。

## RED 与最小实现

原测试在 strong_getter_default_contract_test.rs:122 的 sql_exception().unwrap() 失败，证明缓冲驱动的 LOB 列名更新丢失了既有 SQL 异常（SQLSTATE 0A000）。固定 BASE Filter 恢复新增的只读 ResultSet 默认列名入口提前返回精确 UnsupportedOperation，导致 BufferedRowSet 未进入自身 scalar_update 的 feature_not_supported 错误路径。

BufferedRowSet 具有实际标量和资源 NULL 更新能力。因此为它恢复明确的 update_value_by_label 覆盖：先调用自身 find_column，再调用自身 update_value。关闭/列错误优先级和原更新规则继续由同一驱动验证；没有消费、关闭调用者流，没有修改原测试、增加计数或把 SQL 异常改成通用错误。只读标准默认入口继续保留 SQLite 原 LOB 操作名，未增加第二驱动树或公共接口。

## 实际验证

会话52888终态 exit0，整个 druid-rdbc crate locked/offline/serial 回归：16结果块 / 471 PASS / 0 FAIL / 0 ignored（含1文档测试）。原失败测试实际通过。日志：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-buffered-lob-label-regression.log`。

格式和差异检查通过；标准纯度检查 exit0，STANDARD_FILES=44/PURITY_PHASE=final。

Druid 的完整 diagnostic_tests 回归已启动同一会话39062，日志 task6-buffered-lob-label-druid-regression.log；终态前不宣称27/27或新全workspace通过。此前全workspace3705/3属于修复前，不代表当前源码结果。

## 未完成

迁移账本1092exact/191parameterized，固定1283分母不变；本次修复不新增完成绑定。源断言/调用数量核查1229通过/54缺分支仅为静态证据，完整C2仍首败元数据getter/error/wrapper身份。Callable原创建与关闭计数、Task7八组件、Task8和P05–P07继续开放，共18个顶层任务；Task6仍FIX_REQUIRED。元数据完整草稿尚未接入或编译，不计完成。

Task6 Buffered LOB池化回归终态 2026-10-06：同一39062 exit0；完整diagnostic_tests筛选实际28PASS/0FAIL/0ignored/59filtered，而非预期27，原两个Filter恢复案例实际PASS。日志task6-buffered-lob-label-druid-regression.log。修复后RDBC471/0与此Druid28/0均为已执行范围，不据此宣称新workspace通过。下一完整metadata案例接入TDD；18顶层任务/Task6 FIX_REQUIRED不变。
