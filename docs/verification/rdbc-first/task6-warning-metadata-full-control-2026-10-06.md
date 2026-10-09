# Task6 警告与缓冲元数据完整原控制

## 固定原案例

固定BASE sql_warning_and_result_set_metadata_preserve_structured_fields，body SHA ad9741f72b76d1033862f0a02c98ee11d91db6752b4f4329d6cf9d0aa79a4b74，原assert4/assert_eq7/matches2保留。完整观察为警告message=first、SQLState=01000、vendor code=7、next message=second；两列enabled/optional、Boolean/Unknown、非可空/可空、零下标与列3超过宽度InvalidArgument。

标准映射使用早已批准提前完成的BufferedRowSet代替旧concrete RowSet，沿用同两行值/labels；按canonical ColumnIndex保留1-based参数。零下标控制通过显式Result闭包完成ColumnIndex::new(0)错误传播，第二个越界3实际进入metadata.column_type；不允许把二者都改成构造拒绝。旧is_nullable谓词以同列nullability==Nullable保留，Unknown不冒充Nullable，不弱化原真假断言。

## 已执行证据与边界

同一当前fullworkspace15535/cargo68796仍编译，不改动其Cargo源码输入。独立完整标准消费者warning_metadata_standalone.rs仅存在.superpowers drafts；引用当前RDBC编译库libdruid_rdbc-85c04db4ca3ea2b6.rlib（14:00:30写入，晚于该fullworkspace启动）。独立compile和runtime均实际exit0，完整原控制1PASS/0FAIL/0ignored，日志task6-warning-metadata-standalone-compile.log和task6-warning-metadata-standalone-runtime.log。

此证据说明本完整控制已有正确标准实现，下一步需待同一workspace终态后正式Cargo测试接入、准确绑定和矩阵相关回归，不为创造RED修改现有正确实现。独立消费者不替代正式目标/C2/全仓，不改变账本、固定1283BASE、当前1232/51源观察及全仓测试数量。

完整18顶层任务/Task6 FIX_REQUIRED及Callable原5vs6、剩余ResultSet/Lease/Recycle/Filter、C1/C2/API/SemVer/features/MSRV/资源/发布审查义务仍开放。


## 正式恢复与准确绑定后续

两个完整案例已进入正式lib测试；1309终态exit0/lib93+FilterStat21+matrix6=120PASS/0FAIL。随后准确绑定两条原账本行，固定1283原字段不变，旧矩阵移除两个对应ID；29175绑定后矩阵6/6PASS。全1283审计40618终态1234PASS/49FAIL，登记1097exact/186parameterized。完整C2会话5925exit1推进到result_set_errors_typed_getters_and_old_lease_are_observable。详见task6-object-warning-restoration-2026-10-06.md。全仓15535此前已终态3710/2，本轮无新全仓通过声明。Task6及完整18任务仍未闭环。
