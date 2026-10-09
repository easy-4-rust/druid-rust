# Task6 完整资源读取默认委托恢复（2026-10-07）

完整原合同已通过实际标准过滤器链并正式 EXACT 绑定。Task6 仍为 FIX_REQUIRED，全部 18 个顶层任务保持 OPEN。

## 原合同与实现

固定 BASE 为 `5afe8609b177544ff7d18461aa84f69efe8ee90e`，原 body SHA 为 `60becd81980c7076b7993504428426cf6e29dc66e67e868281ad846b6d7256fc`。原两个 assert 宏体、一个 assert_eq、十三家族的下标/标签二十六次 SQL NULL 观察和二十六条原生事件全部保留。

新增单对象测试夹具 ResourceDefaultsResultSetProbe 和 ResourceDefaultsChainHarness。探针分别观察 Ref、Blob、Clob、Array、URL、RowId、NClob、SQLXML、ASCII/Unicode/Binary stream、Character/NCharacter Reader 的原生入口；标签查找若意外发生也会记录，不隐藏额外事件。桥接调用实际标准 FilterChain，按原资源类型转换返回值，保留原下标与标签，不预读或消费资源。

完整原案例使用实际 FilterAdapter 默认实现。现有生产路由第一次运行即满足原合同；本轮无生产/API 修改，没有虚构产品 RED。只新增完整测试、两个夹具和验证器的精确 lib 目标登记；未放宽验证规则。

## 实际终态证据

- session43175：默认 Rust1.97.1、locked/offline/-j1/test-threads1，完整 Druid lib exit0，146PASS/0FAIL/0ignored。日志 `task6-complete-resource-defaults-initial-2026-10-07.log`，SHA `ac4ca4816453160a8e4d8c1a409abd2ee750caf4cc06bfe5174b0f62e2ab41e9`；1082 项冻结输入终态全部一致。
- session50653：保护式绑定 exit0。完整合同唯一 EXACT，原矩阵占位退役，反向调用索引与派生迁移文档同步；全部 1283 行前七个冻结字段逐项保持。正式 1142EXACT/141PARAMETERIZED。
- session76717：绑定后五个完整消费者目标 exit0，FilterStat21、外部导航1、外部资源1、结果集矩阵1、StrongRead2，共26PASS/0FAIL/0ignored。日志 `task6-complete-resource-defaults-consumers-2026-10-07.log`，SHA `81e0c9944e27841a42e7140e2e7865b50fca35a2a04495d3d14f204ec3bb7cf4`；1082 项绑定后冻结输入终态全部一致。该范围不冒充全 workspace。
- session5620：全1283项源码观测审计完成，1279PASS/4FAIL；审计 exit0 仅说明执行结束，不能报告整体通过。
- session11439：完整 C2 no-execute exit1，下一失败为资源更新默认委托十四重载缺少完整显式案例，整体门禁仍 FAIL。
- 验证器17项自测、final purity48、迁移权威79/pending25/physical73固定SHA、派生文档79、三个文件格式及 diff 检查通过；已有编译 warning 仍保留。

## 未完成范围

当前四个源码合同缺口：资源更新默认十四重载、行修改默认委托、流更新默认二十二重载、下游 close 错误前提交统计。下一资源更新原文与 manifest 已保存，状态为 NOT_COMPILED/NOT_RUN/NOT_BOUND，不计入完成。

Task7 八项义务、API/compiler/SemVer/MSRV/features/完整审查以及 Task6–8/P05–P07 的18顶层任务仍 OPEN。此前全仓3795PASS/11FAIL和独立Admin49PASS/11FAIL为历史失败证据；本轮无当前源码全仓 PASS 声明，也没有更改系统安全规则。

全部本轮句柄已终态。未创建或切换分支，未 stage/commit/push/install/publish。
