# 完整 Statement 关闭与读取峰值合同恢复（2026-10-06）

固定 BASE 案例：`statement_close_closes_traced_result_set_and_records_fetched_peak`，原 body SHA-256 `5d6ac630d84c9fd11a06b7286886635faab78ce6071b0812e7757dd635423abd`。

同一 Toasty SQLite 创建 Statement 执行原 SELECT 1/2/3 查询；两次读取后，通过原创建 Statement 关闭登记资源。测试观察器只委托原 Box ResultSet，并核对实际底层指针、真实 close 调用次数及实际 is_closed，不伪造关闭，不重执行 SQL。

首次59948为观察器 Debug derive 编译错误（0运行测试），移除无关derive。实际6153终态exit101/0PASS1FAIL，观察峰值-1而原要求2；在创建Statement dispose_result_sets的成功关闭路径汇总同一资源实际fetch峰值。33187真实0/1进一步暴露已关闭有效租约next错误；恢复精确Other(result set is closed)，租约失效仍优先ConnectionDiscarded，等待资源锁后再次检查实际租约。5091终态exit0/1PASS0FAIL，原6assert/1eq/1matches全部保留，并验证同一原生close只调用一次。

正式1283分母前7冻结字段与reverse index通过，新增真实lib测试定位，验证器逻辑不变，仅移除对应矩阵占位标识。源码观察1244PASS39FAIL、1107exact176parameterized；C2仍FAIL下一完整强类型getter合同，不能把floor当完整语义证明。

Druid六目标78179终态132PASS0FAIL（lib97、keys2、Buffered7、Callable9、Filter补充16、Filter矩阵1），包括真实底层连接复用后的旧租约完整测试。Wrapper80472终态24PASS2FAIL：两个具名测试共用补充helper，把有效连接中的已关闭游标误判为旧租约。固定原Prepared案例SHA86e3bc7e9eb1717a36e1f3431e0eb3417cb987e7ce71fed43870b9dc5e2ea272未要求该附加错误，原完整Statement关闭案例明确Other；补充控制改为精确Other，并新增connection.close后next为ConnectionDiscarded。22012同四目标最终26PASS0FAIL。两包局部158唯一案例，不重复累计单跑。

Task6 Ruling：native恢复后Filter name/id实际标签不能继续期望缺失；补充控制同时验证真实标签成功和missing准确拒绝。Toasty原SELECT ?元数据标签保留实际?而非占位column_1。原冻结案例断言未删除，补充控制通过不宣称原全部Filter合同完成。

前次全workspace8243终态3733PASS17FAIL/249结果块/exit1已保留；本轮尚待新鲜完整workspace终态。15验证器自测、48 final purity、79 migration authority/25pending/73physical、14 generated matrix通过。命令目标名错误为预检查退出，0运行测试，正确目标重跑完成。

Task6 FIX_REQUIRED、Task7、API/SemVer、MSRV、features、真实数据库、生产运行时与最终审查仍开放；完整18顶层任务未关闭。
