# Task 6 Callable 重载与缓冲行数合同恢复（2026-10-05）

## 范围与状态

完整目标继续为 P04 Task6–8 和 P05–P07。Task6 保持 FIX_REQUIRED；本报告不是 Task6、C2 或发布完成证明。

恢复 BASE `5afe860:crates/druid/tests/callable_statement_semantics_test.rs#prepare_call_overloads_preserve_keys_delegation_and_cache_lifecycle` 的标准接口 helper、显式矩阵分支和独立运行入口。原始 947 行 / 170 assertions 场景的 OUT/named/resource/Calendar/type-map/target-type、执行、Wrapper 继承、参数清理、缓存与统计观察保留，当前 helper 包含 179 assertions。移除通用 CallableProbe 替代场景。测试驱动拆为独立类型文件，mod.rs 仅声明与导出。

实际 factory 记录 SQL、StatementOptions、raw prepare 次数和 raw Callable 状态。测试不通过自行构造缓存键冒充生产观测。标准 typed options 合并 Java 调用形式，原 Precall1/2/3 及未指定 holdability 的精确身份迁移仍开放，不能由此宣称完整重载身份已闭合。

## 实际生产缺口

首次独立运行 exit101 /0 PASS /1 FAIL：`fetched.known_row_count()` 为 None，原有一行缓冲合同期望 Some(1)。CodeGraph 确认 pooled resource 在创建时捕获 raw known_row_count，而 BufferedRowSet 未覆盖默认 None。

在 BufferedRowSet 增加 known_row_count：开放状态按已拥有 rows 返回完整行数，不请求数据库、不推进游标，关闭后 None。补充独立回归覆盖两行前/中/后的计数、首行值不被消费、空结果 Some(0) 和关闭 None。不修改原失败合同期望；不把未知流式结果强行计数。

## 新鲜验证

- `task6-callable-overloads-first.log`：原重载独立测试0/1运行失败，行数 None vs Some(1)。这是生产行为缺口，不是编译失败。
- `task6-callable-overloads-count-fix.log`：同一独立合同1/1 PASS，exit0。
- `task6-buffered-count-contract.log`：新增缓冲计数合同1/1 PASS，exit0。
- `task6-callable-overloads-regression.log`：缓存6/6、FilterStat21/21、Callable4 PASS/2 FAIL，合计31 PASS/2 FAIL，exit101。两个失败仍是既有完整错误 helper 和总矩阵的 prepared5 vs6。
- `task6-callable-overloads-c2.log`：--no-execute exit1，静态检查越过 Callable 区块，下一缺口为 Buffered ResultSet `boolean_getter_returns_bool` 显式分支。1283原始测试分母和 verifier 未改；静态推进不等于运行时等价或全部C2通过。
- final purity STANDARD_FILES43；authority79、Task7 pending25、physical73、SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc，exit0。git diff --check PASS。

全 workspace 新鲜默认 locked/offline serial --no-fail-fast 回归 session50423 已终态，exit101 /237结果块 /3656 PASS /2 FAIL /0 ignored，包含2doctests。日志 task6-callable-overloads-workspace.log；唯一失败目标为既有 Callable 错误独立 helper/总矩阵的 prepared5 vs6。其余目标通过，新增 BufferedRowSet 行数实现和新重载独立测试包含在本次全量结果中。此前3654/2为旧源状态。全部本轮 Cargo/C2 会话均终态。

## 剩余

原普通 Prepared 实际创建后拒绝为 Callable 的统计/释放合同、typed index 原 owner/cache 错误效果、精确调用形式身份、Buffered ResultSet 原用例完整迁移，以及 C1/API/compiler/SemVer/all-features/MSRV/finalreview 和完整 Task7–8/P05–P07 均未关闭。不得把总矩阵前置失败遮蔽的后续用例当作已执行，本次独立测试只证明对应 helper。

最终格式验证：PowerShell 初次将通配符原样交给 rustfmt 导致文件参数错误，不视为PASS；随后显式枚举13个Rust文件，rustfmt --check exit0。新鲜 git diff --check exit0。无提交、推送、发布、安装或分支操作。
