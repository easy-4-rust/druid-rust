# Task 6 Toasty 初始合同恢复（2026-10-05）

状态：部分迁移，Task 6 FIX_REQUIRED。

BASE 为 `5afe860:crates/druid-wrapper/tests/toasty_adapter_test.rs`，不是 toasty_connection_adapter_test.rs。恢复前十项显式分支：默认自动提交、事务提交、回滚、重复 begin 错误、SQLite 六项能力、关闭后清警告拒绝、清警告成功、关闭状态、幂等关闭、关闭 Prepared。保留原始数据与断言，并加强提交/回滚后唯一行 id=1 的验证。重复 begin 原分支已存在，其余九项新增。其他通用 fallback 仍未完成真实合同迁移，不计为完成。

Ruling: 新标准 ConnectionCapabilities 仅保留 transactions/savepoints/catalogs/schemas/callable，旧 auto_commit/read_only/transaction_isolation/holdability 字段不复活；以真实标准操作（关闭自动提交/rollback、只读拒绝、Serializable 设置与 getter、holdability UnsupportedOperation）保存原支持/拒绝语义。Task7 全局 capability 真实性验收仍待完成。

初次编译失败为迁移 API 差异，日志 task6-toasty-initial-contracts.log；不是产品 runtime RED。修正后目标 canonical matrix 1/1 PASS，终态 exit0，日志 task6-toasty-initial-contracts-final.log。矩阵通过不认证未恢复的 fallback 用例。C2 下一轮结果见 task6-toasty-initial-contracts-c2.log。verifier 与 1283 分母未修改。

此前 SQLx 完整 workspace 终态 3646/0 覆盖 SQLx 生产变化；本次随后仅修改 Toasty 测试，已有专项通过，未声称新测试已获完整 workspace 结果。完整 C1/C2/API/compiler/SemVer/features/MSRV/审查及 Task7–8/P05–P07 均未闭合。

最新 C2 终态：session99965，exit1，首个缺失分支推进 `database_meta_data_driver_name`。前十项静态迁移检查已越过；全 C2 仍失败。最终 Toasty 文件格式与 git diff --check 通过。下一步恢复元数据/driver name 的完整原始合同。
