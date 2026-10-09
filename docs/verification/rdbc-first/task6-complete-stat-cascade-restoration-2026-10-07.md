# Task6 完整真实 SQLite 统计与语句级联关闭合同恢复（2026-10-07）

完整原合同已运行并正式 EXACT 绑定。Task6 仍为 FIX_REQUIRED，全部 18 个顶层任务仍 OPEN。

## 合同与迁移语义

固定 BASE 为 5afe8609b177544ff7d18461aa84f69efe8ee90e，原 body SHA 为 5507f632fac9e20bdc0da1430b720f6b2908b97d9fdc2fc70d83007894308160。原 17 个 assert_eq、6 个 assert 全部保留。

同一真实 Toasty SQLite 工厂、同一原生连接、上下文 72/sqlite-stat、第一查询三行及第二查询两行均保留。第一游标验证 int、普通对象、显式 Long 对象、decimal、date、显式空 calendar 的 time 和 UTC timestamp；读取两行后主动关闭，并验证创建语句关闭不重复统计。第二语句复用同一策略、登记表和租约，在读取一行后关闭语句，验证逻辑游标及实际原生游标均关闭。

NativeResultSetCloseObserver 委托原查询登记的同一原生游标，实际执行原生 close 并观察原生 is_closed；关闭前核验原对象地址，未合成关闭结果。补充验证重复关闭语句和结果集不增加原生关闭次数或统计次数。

初始运行 session68558 实际 exit101：139PASS/1FAIL。失败是测试迁移将原普通 Value::Int(1) 错映射为 RdbcObject::Long(1)。修正为标准包装 RdbcObject::Scalar(Value::Int(1))，保留显式 object_as(Long) 的 RdbcObject::Long(1) 断言。原失败日志与 summary 分开保留；这是测试映射修正，不是生产缺陷修复，无新生产接口或路由变更。

## 实际验证

- session73359：默认 Rust1.97.1 完整 Druid lib exit0，140PASS/0FAIL/0ignored；完整原合同实际执行。1,072 项冻结构建输入终态 SHA 全部一致。日志 task6-complete-stat-cascade-green-2026-10-07.log，SHA 5ae2d9759c25deab2ec619cf113856b08751b6c14eac63b83df3a516e912a7e9。
- session50581：保护式正式绑定 exit0；所有 1,283 项原始前七字段逐项保持不变。仅将当前合同改为 EXACT 并退役原参数化占位，验证器规则未放宽。1137EXACT/146PARAMETERIZED。
- session7595：绑定后完整 FilterStat21、canonical ResultSet matrix1、外部资源 Filter1，合计23PASS/0FAIL/0ignored；绑定后1,072项冻结输入 SHA 全部一致。日志 task6-complete-stat-cascade-consumers-2026-10-07.log，SHA 6217d06f4d790476de9a0627825645924c1c198d335b290640098183d7b82d44。
- session9662：全部1,283项原源码合同审计终态 exit0，1274PASS/9FAIL；审计完成不等于全部合同通过。
- session22693：完整 C2 --no-execute 终态 exit1，下一缺口为 result_set_filter_chain_preserves_java_order_short_circuit_and_error_unwind。整体 C2 仍 FAIL，未执行的全量替代测试不作通过声明。
- 验证器自测17/17、final purity48、authority79/pending25/physical73及固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、两个Rust文件格式检查和git diff --check通过。

lib140与消费者23是不同验证范围，分别报告。没有新鲜全workspace通过、API/compiler/SemVer/MSRV/all-features或完整审查通过声明。

## 剩余工作

Task6还剩9项源码合同缺口与其他完整门禁。Task7八义务、Task8、P05/P06/P07及全部18顶层任务继续开放。历史全workspace3795PASS/11FAIL、独立Admin49PASS/11FAIL仍保留；未修改系统安全策略。

下一完整 Filter 顺序合同原文已保存，原8eq/2assert及全部原始事件、精确关闭错误和标量结果43仍需恢复；body SHA0a93c5e3e304e6f82515cc14b0178c00c46aecbef9b5d05b3fd12fb51b463c09。原夹具8块已从固定BASE保存，SHAf4634c8e8113dd1c2c7015ad218629337df076ec54e4fdc96e5e644591ec8857；准备文件仍NOT_COMPILED/NOT_RUN/NOT_BOUND，不计入完成。

本轮所有启动句柄已取得终态；未创建分支、stage、commit、push、安装或发布。
