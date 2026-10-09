# Task 6：恢复结果集创建语句身份与关闭可见性（2026-10-05）

## 范围与状态

计划为 `2026-08-24-rdbc-single-standard-unification.md` Task 6。完整目标仍包括 Task6–8/P05–P07。此报告不宣称 Task6 或迁移完成。

BASE `5afe860` 的 `prepared_result_set_keeps_the_same_statement_alive_and_can_close_it` 已恢复为 canonical matrix 显式分支和独立 runtime test。两条真实 SQLite 工厂（Toasty、SQLx）都执行完整合同：Prepared 叶子与 Statement 父接口、非 Callable、空目标参数错误、未知目标不支持、双向属性变更证明原语句共享、原句柄 Drop 后存活、绑定11的结果读取、重复关闭、兄弟句柄关闭、ResultSet 关闭级联。额外证明游标关闭后创建语句身份仍可访问、游标读取继续拒绝，以及连接归还后 getter 拒绝失效租约。

旧 Any、具体 Druid/Toasty 对象及 Physical 解包断言按批准的单一 standard 规格转换为 canonical Wrapper接口和真实共享操作；没有恢复第二套端口或具体对象泄漏。

## 实际发现与修复

原实现 `DruidPooledResultSet::get_statement` 同时要求结果集与底层资源开放。BASE 在语句级联关闭后仍查询创建语句关闭状态，因此此限制丢失历史合同。

修复仅将该身份 getter 的有效性条件改为当前连接租约仍有效；句柄来自已有 logical owner，无 raw cursor访问、无新资源登记、无重开、无新强引用环。结果集读取和其他游标操作保持关闭检查。

另一条先前迁移测试断言关闭游标后 get_statement 必须失败，与完整 BASE 冲突。已改为验证仍能取得开放的创建语句：more-results关闭游标不关闭语句。此修订不是放宽生产目标，而是以完整BASE合同纠正先前新增断言。

## 验证记录

- `task6-owner-base-migration-red.log`：迁移验证缺少原语句持有/关闭显式 arm，exit1。
- `task6-owner-base-migration-runtime.log`：测试首版把 canonical RdbcObject 与旧 Value 比较，编译失败；不是行为 RED。
- `task6-owner-base-migration-validated.log`：修正类型后实际 RED6/2，独立用例与 canonical matrix 在关闭后 get_statement 返回 ConnectionDiscarded。
- `task6-owner-base-migration-green.log`：修复后新用例 GREEN，但旧新增冲突断言失败；current3/3、generic16/16、matrix7/1。
- `task6-owner-base-migration-final.log`：补齐真实身份/生命周期断言并纠正冲突后，current3 + generic16 + matrix8 = **27/27 PASS，exit0**。
- `task6-owner-base-migration-c2.log`：结构验证发现 assert! 类型断言分母17低于BASE24；补充标准父/叶子/Wrapper赋形、初始状态、关闭身份与失效租约等真实观察。未修改或降低验证器。
- `task6-owner-base-migration-c2-final.log`：通过该完整 arm 后，继续失败在下一条 `prepared_setter_overloads_preserve_exact_descriptor_identity` 缺少显式 arm。完整1283迁移门禁仍未通过。
- 两个变更文件 rustfmt --check 与默认 git diff --check exit0。
- 全工作区默认 serial/locked/offline/no-fail-fast正在运行 session21020，日志 `task6-owner-base-workspace-final.log`；运行终态取得前不声明最新全量通过。

## 下一步

恢复下一条 BASE48参数 setter 描述符合同，保留类型名/日历的省略与指定空值区别、int/long长度及负值、资源不提前消费、clear与1-based参数错误。C1生产证据、全部C2、API/compiler/SemVer/feature/MSRV/full review以及Task7–8/P05–P07仍待完成。

Supplemental static gates: final purity exit0 STANDARD_FILES43; migration authority exit0 rows79/pending25/physical73, SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc.

Task 6 BASE owner-close full workspace terminal verification 2026-10-05: session21020 exit0 /235 result blocks /3632 PASS /0 FAIL /0 ignored incl2doctests, log task6-owner-base-workspace-final.log. Supersedes LIVE21020 note; current get_statement identity fix and complete restored BASE arm included. C2 still fails next setter-overload explicit arm; C1/API/compiler/SemVer/all-features/MSRV/fullreview and Task7–8/P05–P07 remain open. No goal completion or network-reliability certification.
