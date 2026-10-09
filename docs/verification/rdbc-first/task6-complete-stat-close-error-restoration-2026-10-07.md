# Task6 关闭错误前提交统计完整合同恢复（2026-10-07）

原完整合同已通过实际标准链并正式 EXACT 绑定。全部1283项源码观测审计与 C2 静态绑定检查通过；C2 实际执行仍运行中，Task6 仍 FIX_REQUIRED，完整18顶层任务保持 OPEN。

## 原合同与真实观察

固定 BASE 为 `5afe8609b177544ff7d18461aa84f69efe8ee90e`，原 body SHA 为 `173c01e571cfbc58632c9d4a4bf64175be28599521208ab4ccd0463499951c42`。原1个 assert 与7个 assert_eq 完整保留：StatFilter 和 collector 共享同一个 ResultSetStat；开放统计为1，记录三行；下游返回精确 DriverError("close failed")；最终 open=1、opening=0、fetch=3、close=1，context.close_count=0。

新增 StatCloseErrorChainHarness 将原开放和关闭入口委托到实际标准 FilterChain，未伪造统计。StatCloseErrorResultSetProbe 在实际原生 close 入口、错误返回前记录同一 collector 的 `1:0:3:1`，并验证原生日志仅一次 physical:close。固定 BASE 的探针在返回错误前已标记 closed，此行为保留并实测。

现有生产路由第一次运行满足合同，没有修改生产代码、公开 API 或验收规则，没有虚构产品 RED。验证器仅登记精确 lib 测试目标。

## 实际终态证据

- session42552：默认 Rust1.97.1、locked/offline/-j1/test-threads1，完整 Druid lib exit0，150PASS/0FAIL/0ignored。日志 `task6-complete-stat-close-error-initial-2026-10-07.log`，SHA `baa702d2e3f848aa332d4d6a606872abb6369a06db73a6a1dc34deeecb76f530`；1090项冻结输入终态全部一致。
- session10807：保护式唯一 EXACT 绑定 exit0，全部1283行前七冻结字段逐项固定；仅退役该案例矩阵占位，反向调用索引和派生文档同步。正式1146EXACT/137PARAMETERIZED。
- session36178：绑定后六个完整消费者目标 exit0，FilterStat21、外部导航1、外部资源1、矩阵1、外部行操作1、StrongRead2，共27PASS/0FAIL/0ignored。日志 `task6-complete-stat-close-error-consumers-2026-10-07.log`，SHA `03030f0ce561e1b33349fdd1679f0b3ee408b02e0aeaf068a4aad2f39c2c4dbb`；1090项绑定后冻结输入全部一致。
- session62239：完整源码观测审计 exit0，1283PASS/0FAIL，固定分母与全部前七字段不变。JSON 为 `task6-complete-stat-close-error-source-floor-audit-2026-10-07.json`。
- session75961：完整 C2 `--no-execute` exit0，67删除文件、1283测试、1146EXACT/137PARAMETERIZED、pure_identity=0/approved_removal=0、zero_test_identity=1/zero_test_behavioral_handoff=1。输出明确 executed=no，不能证明实际执行完成。
- 验证器自测17、final purity48、迁移权威79/pending25/physical73固定SHA、派生文档79、三个文件格式及 diff 检查通过。既有 warning 未冒充清零。

## 完整 C2 实际执行

session37151 已启动且确认 LIVE，运行原验证器默认执行路径，未使用 --no-execute；逐目标 all-features/locked/offline 执行，并检查每个精确函数和参数化案例 digest。Cargo并发与测试线程各1，工具链1.97.1，1090项输入已冻结。日志为 `task6-complete-stat-close-error-runtime-2026-10-07.log`，冻结文件为 `task6-complete-stat-close-error-runtime-inputs-2026-10-07.json`。目前无终态和通过声明，须继续同一句柄，不能因安静或观察超时重启。

## 未完成范围

源码观测缺口现已关闭；Task6 的完整实际执行、C1/API/compiler/SemVer/MSRV/features/最终审查仍待验收。Task7八义务、Task8及P05–P07的18顶层保持 OPEN。

此前全仓3795PASS/11FAIL及独立Admin49PASS/11FAIL继续保留；没有当前全仓PASS、网络故障已解决或发布完成声明。未创建或切换分支、stage/commit/push/install/publish，也未更改系统安全配置。

## 后续终态更正

上述 session37151 的 LIVE 描述已被后续实际终态替代：exit1，成功函数标记被 nocapture stdout 打断，1090构建输入全部一致。真实目标两种输出模式均7PASS/0FAIL；修复验证器输出方式后完整C2在session33663重跑，本次核对仍LIVE，没有整体通过结论。详见 task6-final-gate-runtime-recovery-2026-10-07.md。此前150/0、27/0和1283源码审计证据范围不变。
