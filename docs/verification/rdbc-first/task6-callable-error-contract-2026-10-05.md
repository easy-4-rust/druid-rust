# Task 6 Callable 错误合同恢复（2026-10-05）

状态：PARTIAL / FIX_REQUIRED，完整目标矩阵仍为 RED。

## 原始验收目标

BASE `5afe860:crates/druid/tests/callable_statement_semantics_test.rs#callable_errors_invalidate_cache_and_non_callable_handles_are_rejected` 要求非法索引、22 个空名称 setter、missing named OUT、错误 OUT 类型、byte 溢出和非 Callable/unsupported prepare 拒绝；driver prepare_count=7，pool prepared_statement_count=6、cached_prepared_statement_delete_count=6、closed_prepared_statement_count=6。

本轮恢复完整测试骨架及独立运行用例，保留原始错误检查和三个统计断言。标准 typed selector 与资源/stream overload descriptor 映射后，原先通用 CallableProbe 正常参数测试不能代替该用例。该矩阵其余三个分支仍未完整恢复。

## 实际发现与修复

首次编译失败缺少 Connection trait import，保留 task6-callable-errors-red.log。补 import 后测试 acquisition 挂起：MySQL checker 需要 create_statement，fixture 未实现。核实并停止本轮测试 PID51356（完整 exe 路径匹配），session90983 终态0xffffffff。这是准备失败，不是运行时行为 RED。补齐普通 Statement 的校验行为，未调整池超时/重试/生产校验策略。

真正运行 RED：task6-callable-errors-validated-red.log，0 PASS /2 FAIL，空名称 String setter 被接受。生产 DruidPooledCallableStatement::set_named_parameter 在 ensure_open 后显式拒绝空名称；规格已先记录 BASE 要求。独立 scalar/Blob/AsciiStream 验证通过。完整原始辅助用例和 canonical matrix 仍失败，但已推进到 prepared_statement_count：实际0、要求6。

最新完整目标：task6-callable-name-validation-next-red.log，1 PASS /2 FAIL，exit101。此前空名称、missing OUT、错误类型、溢出及prepare_count=7断言已执行通过；不能因此宣布全合同通过。

相关既有合同回归：task6-callable-name-validation-regression.log，OUT参数5 +池化Callable14 +标准转换2，共21 PASS /0 FAIL，exit0。两文件fmt、purity43、authority79/pending25通过。

## 必须继续解决的迁移边界

- 当前 Callable raw close 未纳入原始 Prepared 缓存统计，准备计数为0。这是生产缺口，不能修改预期6规避。
- 非法 index 在 ParameterIndex/CallableParameter 构造时拒绝；如何维持原始 owner 的缓存失效责任尚未证明。当前测试验证同一输入拒绝，但不认证索引错误的 owner/cache合同。
- 当前 ordinary-handle fixture 返回规范的 UnsupportedOperation，尚未证明真实 Prepared-only raw 的标准 Wrapper 可赋值拒绝。要补真实类型/编译证据，不能把 fixture拒绝当作完整下转型证明。
- 其他三个 Callable BASE case（generic ordered、result owner identity、prepare overload/cache）仍需完整恢复；全 C1/C2 未闭合。

Ruling: 保持原始统计6及全部拒绝断言，把typed selector与nonCallable迁移证据不足明确留为待办。当前不关闭此case，也不推动Task7验收。空名称校验恢复在逻辑层，不交由 permissive driver 决定；其他合法名称的既有21项回归通过。

未运行新的全workspace；最近3646/0是此前SQLx源码，不能代表本轮生产改动已全量GREEN。完整Task6–8/P05–P07范围保留；未提交/推送/发布/安装。
