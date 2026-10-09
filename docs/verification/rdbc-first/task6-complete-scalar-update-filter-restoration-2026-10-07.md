# Task6 完整标量更新 Filter 合同恢复（2026-10-07）

原 pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors 完整恢复并正式 EXACT 绑定。Task6 仍 FIX_REQUIRED，18 顶层任务仍 OPEN。

## 原合同与实现

固定 BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原 body SHA 668699380f3bc0a8808f2a6980e38270b8502aab27ebb0e4274ae982fb11d184。原6个assert_eq与1个assert及宏展开28次默认错误观察全部保留，额外逐项校验28个有序短路事件。

保留同一真实 Toasty SQLite factory、原上下文87/88、数据源名、SELECT 1 AS value，以及14类下标/标签更新。日期2026-07-29、时间10:11:12、BigDecimal 12.340及默认1、正负数字、Some/None、字节0/255全部保留。标签fail产生精确 filtered scalar update failure，实际创建Statement异常计数1；默认Filter保留原两种 UnsupportedOperation 操作名及实际异常计数28、游标开放状态与显式关闭。

新增单对象 ScalarUpdateShortCircuitFilter 对标准 ResultSetUpdate 描述符逐类匹配，保留原事件名/参数/标签错误；非标量继续同一标准链。测试通过标准 Statement 执行查询，使用实际池化 ResultSet。现有生产路由已经支持，无生产代码或公开API变更。原方法标识保留在宏映射中，实际执行 canonical 描述符，未恢复第二接口。

## 实际验证

首次 Box 动态结果集和后续具体池化对象尝试均暴露旧标量方法不存在，各57个编译错误，两个原日志与summary分别保留。这些是测试接口迁移失败，不是生产runtime RED。随后与现有标准合同一致改用类型化描述符，未弱化原观察或为测试增加生产API。

45261 默认Rust1.97.1完整Druid lib终态exit0：138PASS/0FAIL/0ignored。完整标量更新原合同实际执行通过，1070项Rust/Cargo/proto/SQL构建输入SHA在终态不变。日志 task6-complete-scalar-update-filter-green-2026-10-07.log，SHA ebdd522a5f196b7c9914d474c5fadec06345164b36896d81107b74ecf3ce1c5e。

正式绑定后两个完整消费者目标（FilterStat21及canonical ResultSet matrix1）22PASS/0FAIL/0ignored，绑定后1070项输入SHA不变；日志 task6-complete-scalar-update-filter-consumers-2026-10-07.log，SHA fff5b0fab9f0077d6e65475a5dd5f5bc54301adbef9c11f166f88069a107992c。与lib138分开报告。

88323保护式绑定exit0，全部1283前七冻结字段逐项验证不变，只移除此原标量更新矩阵占位、窄注册实际lib入口。1135EXACT/148PARAMETERIZED。16519完整源码观察审计exit0：1272PASS/11FAIL。51257完整C2 --no-execute终态exit1，下一完整流更新原case缺少显式case；整体C2仍FAIL，未降低验证器规则。

验证器自测17/17、final purity48、authority79/pending25/physical73固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、三Rust文件格式检查通过。没有当前全仓通过或全feature/MSRV/API/SemVer通过声明。

## 剩余范围

Task6剩余11源码合同；Task7八义务、Task8/P05/P06/P07及18顶层任务继续开放。历史全仓3795PASS/11FAIL、Admin49PASS/11FAIL保留，未调整系统安全策略。下一流更新原合同已保存，body SHA52c19b7021c08421083fb9132badc1afc2fca0d753ab6e4fda7efd5c6dd1295f，NOT_COMPILED/NOT_RUN/NOT_BOUND，不计入完成。

本轮所有启动句柄已终态，没有stage/commit/push/分支/安装/发布。
