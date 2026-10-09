# Task6 完整 RowSet 导航合同恢复（2026-10-06）

固定 BASE：5afe8609b177544ff7d18461aa84f69efe8ee90e。
原案例 row_set_result_set_preserves_labels_navigation_and_validation，body SHA256：2ad9cb3c11788fa6f27129f14d0e8a99fd75f976196caf90ad4376ad41f9e0c8。

完整案例位于 crates/druid/tests/task6_complete_row_set_navigation_contract_test.rs。
保留大小写标签、NULL、首尾/绝对负数/相对导航、before/after 边界、JDBC 属性代码、非法标签/零列/999方向/负 fetch size，以及关闭后精确 Other(result set is closed)。补充非法输入不改变游标/属性、枚举代码往返和构造默认 Forward1000。

## 实际 RED 与修正

接口接入21编译错误单独保存在 task6-navigation-formal-red.log；不是运行失败。
后续真实运行1PASS1FAIL、1PASS2FAIL分别证明关闭错误和默认方向偏离 BASE。
生产恢复仅限 BufferedRowSet 原生默认 Forward 和精确 Other；四个描述符 rdbc_code 与 FetchDirection.TryFrom 保留 JDBC 身份，未引入第二套原始整数驱动端口。

较早新增的 typed scalar、date、decimal 补充测试误将原生缓冲关闭合同写成 pooled/adapter ConnectionDiscarded。BASE object_as/date/decimal 传播 value 的原始错误，RowSet.ensure_open 明确返回 Other。按基线只修正三条精确预期，不放宽为 is_err；保留真实失败日志 task6-navigation-green.log（历史文件名虽含 green，实际140PASS1FAIL）与 task6-navigation-final-related.log（149PASS1FAIL，后续目标未执行）。

## 新鲜验证范围

- 修正后的 Druid 七目标：141PASS0FAIL，task6-navigation-related-corrected-green.log；绑定后同七目标在 task6-navigation-final-related.log 中实际141PASS0FAIL，其后RDBC补充冲突单独处理。
- 修正后的 RDBC 四目标：444PASS0FAIL；Wrapper buffered_fetch_size_contract_test：3PASS0FAIL，task6-navigation-consumer-corrected-green.log。
- 本轮相关合计588PASS0FAIL；不等于 workspace 全量结果。
- 固定1283账本仅改一个案例的迁移绑定；1101exact/182parameterized，全部原始前七字段不变。
- 完整源码观察审计1238PASS45FAIL，task6-navigation-source-floor-audit-2026-10-06.json；不等于运行证明。
- C2 --no-execute exit1，下一缺口 sparse_result_set_navigation_defaults_report_exact_capabilities。
- 检查器自测15、标准纯度48文件、权威79行/Task7待处理25行/Physical映射73行、生成证据14文件、格式和diff检查通过。

## 未关闭范围

最近完整workspace3716PASS2FAIL属于较早源码，失败为Callable创建计数5vs6；本轮没有新完整workspace验收。新公共枚举查询和TryFrom仍需API/SemVer、默认编译器/MSRV、features与整体审查。Task7八义务与Task8/P05–P07保留，完整18顶层任务仍未关闭。没有安装、切分支、提交或发布。
