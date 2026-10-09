# Task 6：隐藏列与共享列映射合同恢复（2026-10-05）

规格事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
执行计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md`，Task 6。
完整目标仍为 P04 Task 6–8 与 P05–P07；本报告不声明 Task 6 或整个计划完成。

## 实际问题与修复

已有真实 SQLite 开启 Filter 测试在已配置映射中查询缺失键 99，实际为 `Some(99)`，BASE 合同要求 `None`；原始失败日志为 `task6-result-column-map-red.log`，0/1，exit 101。正反方向此前的 `unwrap_or` 都将未配置与缺失键混为一谈。

共享游标状态中的正反映射与隐藏列改为独立 RwLock。未配置映射仍采用恒等映射；已配置映射只接受存在的键。正反设置器独立清除，隐藏列按驱动列判断。开启 Filter 的初始配置和后续内部设置器共用真实状态入口，配置完成后才发布共享视图；锁不跨越 await。

隐藏列与映射恢复为 Druid 内部具体方法，不重新公开旧驱动端口或第二套 standard。隐藏列 getter 返回拥有型快照，以适应共享状态的 Rust 所有权。

## 原始测试合同与验证边界

恢复 5afe860 的 `druid_pooled_result_set_differential_test.rs` 三项完整合同：

- hidden_columns_none_by_default：初始零/None、Some([3,4])/计数二、清除后 None。
- physical_column_identity_without_map：正反列 1 都为 Some(1)。
- set_logic_column_map_and_reverse：1→2/2→1、缺失 99 为 None、反向 2→1、清除正向后恒等。

测试执行原五行 SQL，使用实际 Toasty SQLite 驱动、标准资源准入/登记、Druid Statement 与 ResultSet 生产构造器；同资源的第二视图观察并修改同一状态。额外证明隐藏列拒绝读取、清除后恢复实际 1.5、映射读取真实 row1、正反映射独立清除。此处不冒充完整 native pool 借用/回收验证。

首次 GREEN 尝试为 10 PASS/1 FAIL：额外标签查询成功断言与原 SQLite 不支持标签查询的边界冲突。修正额外断言为准确 UnsupportedOperation，使用实际 logical_column_index 验证反向转换；没有修改驱动标签行为或放宽原三项断言。日志 `task6-result-column-map-green.log` 保留，不作为新生产缺陷 RED。

最终诊断组 11/11 PASS，exit 0，日志 `task6-result-column-map-final.log`。

## 迁移分母与门禁

三项 BASE 行改为明确的完全限定私有 lib 测试入口，保留原身份、正文 SHA 与总分母 1283；去除 canonical matrix 中相应通用 fallback case。当前登记为 1075 exact /208 parameterized，只是登记数量，不是完成率。

校验器仅增加这三个明确 lib 名称，未放开任意生产源码目标。反向调用索引和既有生成文档区域已同步；校验器自测 15/15 PASS。

final purity：43 个标准文件 PASS。
迁移权威总账：79 项，Task7 pending25，Physical 分母73；SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc` 未变，PASS。
C2 --no-execute：exit1，推进至 `int_getter_returns_integer` 缺少明确分支；完整 C2 仍未通过。日志 `task6-hidden-column-maps-c2.log`。

完整 workspace 回归正在执行，日志 `task6-hidden-column-maps-workspace.log`；终态将在本报告追加。旧 3662/2 全量日志早于本次生产修改，不能证明当前状态。

## 剩余工作

Task6 保持 FIX_REQUIRED：完整 C1/C2、Callable 原 prepare5vs6、typed index owner/cache、普通 Prepared-only 拒绝、调用重载身份及 API/compiler/features/MSRV/review 门禁仍开放。下一项恢复原 int getter 与标签错误合同，不以通用缓冲 smoke test 替代。Task7 八项剩余组件与 Task8/P05–P07 范围不变。

## 完整回归终态

当前源码的 default workspace locked/offline/serial 回归已终止：exit101，239 个结果块，3673 PASS /2 FAIL /0 ignored，含两个 doctest。唯一失败目标为 Callable canonical matrix，两个失败函数：callable_errors_preserve_full_base_cache_and_parameter_contract, task6_callable_canonical_matrix；均为原有 prepared5vs6 合同。新增11项诊断与其余 default targets 通过。日志 `task6-hidden-column-maps-workspace.log`。此终态替代本报告运行中说明；不代表 all-features、MSRV 或发布门禁通过。
