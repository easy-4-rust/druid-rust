# 三个错误对象的标准归属恢复（2026-10-08）

结论：三个真实错误对象迁入 RDBC canonical error 模块，旧 compat 和 Druid 门面重导出同一类型。Task6 FIX_REQUIRED、Task8 INCOMPLETE；全部18项顶层任务仍开放。

## 实现与合同

- BatchUpdateException 保留 SqlException、SQLState、vendor code、异常链和原始 i64 更新计数；i32 投影遵守 Java 窄化转换。
- DataTruncation 保留 index、parameter、read、未知 data_size 和 transfer_size。
- SqlClientInfoException 保留大小写敏感的失败属性及 vendor 原始状态字符串，没有改成另一种 payload。
- 三个实际定义分别位于 error/batch_update_exception.rs、error/data_truncation.rs、error/sql_client_info_exception.rs。compat/exceptions.rs 删除重复定义，仅重导出。
- error_contract_test.rs 保留原6项，新增3项跨路径身份与 payload 回归。

## 真实验证

| 检查 | 结果 |
| --- | --- |
| canonical 导入 RED | 实际 exit101，E0432 缺失三个标准公开名称 |
| 完整 RDBC all-features 回归 | 原89913 exit0，477PASS/0FAIL，含9项 error contract及doctest |
| Druid lib与身份回归 | 原85298 exit0，152PASS/0FAIL |
| workspace fmt | 实际 exit0 |
| final source purity | 实际 exit0，STANDARD_FILES=49；仅认证脚本覆盖的源码范围 |
| 严格 workspace Clippy复验 | subprocess直接记录exit0，all-targets/all-features/locked/offline/-Dwarnings |
| 69行原始外部探针全量复编译 | 41PASS/27FAIL，1项非Rust目标未认证；同一原始探针源码 |
| 冻结输入复核 | 1097源码+58控制输入，变化0 |

原Clippy89186日志记录1m52s正常完成，但句柄回收后未补取其退出码；保留原日志，以clippy2直接捕获exit0作为终态证据。

[机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-error-canonical-ownership-recovery2.summary.json)

[全量探针](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes2.summary.json)

## 剩余边界

canonical名称/身份通过不认证全部Java方法、最终API/SemVer、真实数据库或全workspace runtime。语义inventory和oracle未修改，没有手写LIVE晋升。当前27项编译诊断、final CLI、Task7八项义务及P05–P07继续开放。
