# Task 6 Toasty adapter 69 项原始合同迁移（2026-10-05）

状态：本矩阵运行通过，Task 6 仍 FIX_REQUIRED。

## 原始分母与恢复内容

BASE `5afe860:crates/druid-wrapper/tests/toasty_adapter_test.rs` 的 canonical 矩阵共有 69 个 case id；本轮在前十项已恢复的基础上，补齐其余元数据、驱动名称、exec/batch、Prepared、查询及值参数、强制失效、ping、连接复用、自动提交、read-only、隔离级别、warning 合同。五个既有保存点分支也恢复 BASE SQL 与原始名称。所有 69 个 case 都有单独 literal match arm；删除原 `_` 分支里通用插入数据逻辑，未知 case 直接 panic，避免以后新增名称未经测试就进入 digest。

保留 BASE 的数据、步骤、错误种类和断言；额外核对 Prepared batch 三行值、更新后的值、generic ResultSet 值、查询顺序及 EOF。原始 eager fetch_prepared 映射标准 owned cursor，逐行消费保持返回行数及内容；Prepared 关闭操作直接属于其 owned standard handle，不重建 connection-owned 私有端口。

## 裁决与边界

Ruling: 原始 mark_discarded/is_discarded 的失效语义映射 canonical abort/is_closed，并观察执行返回 ConnectionDiscarded。当前 direct Toasty abort 委托 close，完成 raw 连接失效；不恢复 retired 私有接口，不据此认证所有 driver 的取消治理。

Ruling: 原始 callable case 明确允许成功或失败，只要求不 panic。迁移使用标准 prepare_call，成功时关闭句柄，错误保留原允许范围；不把该用例标记为 callable 能力认证。

Ruling: 原始 Arc 结果集用例仅要求返回打开的结果集；迁移使用标准 owned ResultSet，保留打开状态并加强真实行值/EOF，而不新增第二游标接口。

前轮旧 capability 字段已通过标准行为映射，本轮不重新引入 Physical descriptor。Task7 全局 capability、资源和取消门禁未关闭。

## 实际验证

- 元数据与执行 16 项恢复后：task6-toasty-metadata-execution.log，目标 matrix 1/1 PASS，exit0；C2 静态推进 execute_prepared_on_closed_statement_errors。
- 查询与类型 18 项恢复首次编译失败：ColumnIndexes 要求 typed ColumnIndex，日志 task6-toasty-query-values.log；这是 API 迁移编译失败，不是产品行为 RED。
- 修正 typed index 后：task6-toasty-query-values-final.log，matrix 1/1 PASS，exit0。
- 最终剩余连接状态合同与五个保存点恢复：task6-toasty-complete-adapter-matrix.log，matrix 1/1 PASS，exit0，实际执行全部 69 case。
- 命令：cargo test -p druid-wrapper --test task6_toasty_canonical_matrix_test --locked --offline -- --test-threads=1。
- 本轮未修改生产 Rust。格式检查、git diff --check、purity43 与 authority79/pending25 通过。
- 最新 C2 no-execute 正在运行 session78686，日志 task6-toasty-complete-adapter-c2.log；终态另行追加。

此前完整 workspace 3646/0 属于 SQLx 资源恢复后的源码，不覆盖本轮新增 Toasty 测试；当前修改已有完整目标矩阵运行证据。未重跑无关全量以代替具体缺口修复。

完整 C1/C2、API/compiler/SemVer/all-features/MSRV/整个 Task6 审查仍未完成；Task7 八组件、Task8、P05–P07 全范围保留。没有 commit/push/publish/install/分支切换。

最新 C2 终态：session78686 / exit1，首个缺失分支推进 Callable `callable_errors_invalidate_cache_and_non_callable_handles_are_rejected`。本轮 Toasty 分支已越过静态检查；全 C2 未完成。所有本轮进程已取得终态。
