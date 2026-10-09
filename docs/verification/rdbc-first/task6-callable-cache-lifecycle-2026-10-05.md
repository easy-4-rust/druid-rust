# Task6 Callable 共享缓存生命周期（2026-10-05）

状态：缓存与失败路径专项已验证；完整 Task6 仍为 FIX_REQUIRED。

## 实现与可观察合同

Callable 原先直接 prepare/close，不经过 Prepared 的连接级 LRU。现在缓存私有条目用 CachedPreparedStatement 保留 Prepared/Callable 真实 owned 标准类型，两种句柄共用一个容量、access-order、淘汰及失败关闭队列；没有新增公共 raw/Physical 接口。缓存键继续区分 SQL、catalog、语句种类与 typed result-set options。

Callable 在关闭时先结束子结果，恢复底层初始属性，再清理参数与两类 batch；可复用句柄进入缓存，否则关闭并记录原始 Java 统计。连接归还通过已有 shared logical owner 完成同一路径，旧逻辑句柄失效，下一租约可复用 raw。registry 直接释放 raw 时也记录计数，正常 take 后不会重复关闭。

OUT 的 31 个标准读取/转换方法直接委托同一底层 Callable，保留 type map、target type、Calendar/scale 参数。getter 转换、参数变更及执行错误使句柄不可缓存；fresh failed 保留 cached=-1 的既有 Java 合同，checked-out failure 使用已借出条目账务。

## TDD 与验证证据

- task6-callable-cache-red.log：4 个真实生命周期测试均失败，分别证实立即 raw close、不复用、跨租约重新 prepare、options 不能复用。
- task6-callable-cache-first-green.log：编译失败（错误 import、子模块方法可见性），不作为运行时证据；修复后 task6-callable-cache-validated-green.log：4/4 PASS。
- task6-callable-cache-errors-red.log：4 PASS /1 FAIL，空名称错误后 raw 仍进入缓存。
- task6-callable-cache-errors-green.log、task6-callable-cache-errors-validated-green.log：getter 委托生成与 CurrentResultDisposition import 编译失败，单独保留，不计为行为 RED。
- task6-callable-cache-errors-final.log：5/5 PASS，空名称、missing OUT、wrong type、byte overflow 和 generic prepared execution failure 都不复用。
- task6-callable-cache-execution-red.log：4 PASS /1 FAIL，prepared-query failure 仍复用。
- task6-callable-cache-execution-green.log：5/5 PASS，进一步覆盖 prepared query/update/batch、继承 query/update/batch、generated keys 和 current getter 共 13 个错误路径。
- task6-callable-cache-out-regression.log：缓存专项5、OUT描述符5、Callable代理14、转换2、Prepared LRU5、Prepared cleanup7 全通过；原始错误 helper/矩阵2个失败仍为 prepared count 5 vs 6，独立空名称测试通过。无失败遗漏，也没有把期望6改为5。
- 最终源码格式化、git diff --check、标准纯度43、迁移authority79/pending25通过；完整 workspace 回归以进度台账中的终态记录为准。

日志保存在本计划 .superpowers/sdd/2026-08-24-rdbc-single-standard-unification/，不是发布证据。

## 尚未关闭的原始合同

1. 原始 ordinary-handle factory 实际创建普通 Prepared，随后 Druid 拒绝 Callable 转换，因此原始创建/关闭计数包括第6个句柄。当前标准接口 fixture 提前返回 UnsupportedOperation；它没有实际创建句柄。不能仅按错误种类伪增计数，也不能把 fixture 预设拒绝当成真实 Prepared-only assignability 证明。
2. typed ParameterIndex 在构造时拒绝0，原始 indexed setter/getter 的逻辑 owner 不会收到该失败；owner/cache 等价性仍需完成明确迁移证据，不能凭构造错误假称已恢复。
3. 原始4个 Callable BASE case 的其余完整内容仍需恢复；本文新增缓存专项不替代完整 overload/delegation、ordered results、dynamic owner identity 或 C1/C2 验收。
4. 当前 typed StatementOptions 合并了 Java 调用形式；这里只证明有效 options 分区，不声称显式默认参数调用与无参数调用的原始重载身份已全部等价。
5. 取消安全、所有 driver 的 exactly-once 释放和资源/stream 认证仍属于 Task7，本文不提前关闭。

Ruling: 使用私有枚举保留真实 owned trait 对象并共享既有 LRU，禁止另建 Callable cache、独立容量或第二公共端口。风险：若仅以计数通过验收，句柄复用与原始 owner 错误副作用可能仍不等价；故本轮保留完整原始失败，未关闭 Task6。

## 全量回归发现与后续修复

首次完整 workspace：task6-callable-cache-workspace.log，session73341 terminal exit101，236结果块，3650 PASS /3 FAIL /0 ignored（含2doctests）。失败为完整 Callable helper/矩阵（5vs6）和 canonical_prepare_rewrite_cache_parameters_batch_events_and_timeout_match_raw（新增 Callable 正确恢复初始 timeout=0，旧观测只包含 Prepared 恢复）。按已批准 Callable 继承 Prepared 属性恢复合同补齐最后一次0，不删除原始观测。

独立审查发现执行 policy 已分类致命错误，再次 classify 会重复 sorter/fatal handler。新增实际 MySqlExceptionSorter 计数委托测试：task6-callable-fatal-once-red.log，5 PASS /1 FAIL，generic classified2vs1。执行和已分类继承 getter 改为仅记录 cacheable，raw mutation/OUT/direct keys仍分类一次。task6-callable-fatal-timeout-final.log exit0：缓存生命周期6/6 +完整 Filter/Stat21/21 PASS，7条 fatal execution 路径均分类一次、discard一次、raw关闭一次。

最终全量以 task6-callable-cache-workspace-final.log 的终态为准；首次全量不代表修复后的源码。C2 task6-callable-cache-c2.log terminal exit1，仍失败于 Callable generic ordered results 显式迁移分支，未关闭。

最终完整 workspace（session65035）exit101，236结果块，3652 PASS /2 FAIL /0 ignored，含2doctests。仅原始 Callable helper/矩阵在 prepared5vs6 失败，其他默认目标全部通过。该命令覆盖最终生产与测试源码；不推导 MSRV、all-features、真实数据库全矩阵或生产可靠性完成。全部已启动句柄终态，完整计划仍未完成。
