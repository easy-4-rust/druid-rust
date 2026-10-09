# Task 6 当前结果 getter 的拥有型交接（2026-10-05）

状态：getter 恢复目标测试通过；完整 Task 6 仍 FIX_REQUIRED。总目标保持完整 Task 6–8 与 P05–P07。

## 实际行为与设计

- DruidPooledStatement 在已有结果序列时仍调用标准 driver get_result_set，真实错误交给现有 policy 分类；只有精确默认 statement_get_result_set UnsupportedOperation 且已有逻辑序列时允许逻辑替代。
- PreparedStatementView 补充 raw getter 委托；CallableStatementView 补充 raw getter/count/more-results 委托。未新增第二个 driver port。
- driver 返回同一当前游标的拥有型视图时，StandardResultSetResource#adopt_current_view 在其已有 async cursor mutex 中接管并替换 raw view。getter 返回资源继续使用同一 registry registration、shared state、Filter context、owner 和混合结果序列，不重复 open。
- 旧视图被新拥有型视图替代时不执行 cursor close；关闭统一由原登记资源处理。已关闭资源不复活；返回已关闭的 raw view 不再次 close，仍开放的交接视图须释放并分类错误。
- 新 fixture 的 cursor state 只包含数据和操作记录；普通、Prepared、Callable 返回拥有同一 cursor state 的不同 raw views。next 记录具体视图来源，证明新 getter 视图实际被接管，不是将 Some 丢弃后继续用旧 raw。每个新增测试对象独立文件，mod.rs 只声明和重导出。

Ruling：当前 getter 返回的是同一 driver cursor 的拥有型视图，逻辑身份归原登记资源；替换驱动视图而不重新包装逻辑资源，满足既有 JDBC 当前结果身份合同。保留 prior four-getter RED，不能通过丢弃 Some、重复 open、重置混合序列或引入另一驱动协议来宣称完成。第三方 driver 本身提供同一当前 cursor 的正确视图仍是其标准实现责任；本次受控三种语句和真实 SQLite 证据不替代所有外部 driver 认证或 Task7 取消/流式合同。

## RED/GREEN 与验证

- task6-getter-owned-handoff-red.log：最初测试写错 FilterChain 方法名和 Arc 入参，编译失败，不计行为 RED。
- task6-getter-owned-handoff-runtime-red.log：实际0/1 FAIL，ordinary getter_calls0，生产跳过 raw getter。
- task6-getter-owned-handoff-green.log：exit0，current-result3/3 + canonical prepared7/7，共10/10 PASS。包含上一轮完整 BASE MySQL 四 getter failure 的 runtime closure。
- 成功 handoff 合同在 ordinary/prepared/callable 三路径覆盖两次 raw getter、原始/新 getter 交替推进7→8、raw getter 来源 next、原视图 Drop、新旧 Filter ID相同、无重复 open、UpdateCount2、下一结果20、终态None及重复false、原始/别名/statement/connection/pool close 后每个 raw cursor close 调用恰好一次。
- 已补充 getter 返回已关闭 raw view 的不复活/不额外 close 检查，未据首次通过伪称新的 RED/GREEN。当前完整 workspace 命令包含该补充。
- task6-owned-handoff-c2.log：no-execute FAIL，下一待恢复显式 case 为 prepared_result_set_keeps_the_same_statement_alive_and_can_close_it。四 getter arm 全运行通过不代表 C2 1283项迁移完成。
- purity final STANDARD_FILES43 PASS；authority79 / Task7 pending25 PASS，SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc。
- 新增 support 模块及四生产文件/目标测试 rustfmt检查与 default diff PASS。
- 全 workspace default locked/offline serial --no-fail-fast LIVE session2854，log task6-getter-owned-workspace-final.log；终态前不声明全回归通过。上一轮3628/2对应旧 get_result_set 实现。

## 剩余

继续完整 BASE owner identity、参数 descriptor、清理失败和 C1/C2 行为迁移；API/compiler/SemVer/all-feature/MSRV/整体审查、Task7–8、P05–P07均未完成。没有 stage/commit/push/publish、分支切换或工具安装。

Task 6 current-getter workspace terminal verification 2026-10-05: session2854 terminal exit0 /235 result blocks /3631 PASS /0 FAIL /0 ignored incl2doctests. Log task6-getter-owned-workspace-final.log. Includes supplemental closed current getter no-revive/no-extra-close contract. Supersedes LIVE2854 notes; default suite success does not close C1/C2/API/compiler/SemVer/features/MSRV/full review or Task7–8/P05–P07. Next complete BASE explicit case is prepared_result_set_keeps_the_same_statement_alive_and_can_close_it.
