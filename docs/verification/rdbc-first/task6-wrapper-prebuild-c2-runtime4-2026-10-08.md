# Task6 全功能 Wrapper 终态与完整 C2 runtime4（2026-10-08）

## 已取得实际终态

原预构建 session45521 真实exit0：`cargo test --offline --locked -p druid-wrapper --all-features --lib -- --show-output`，16 PASS、0 FAIL、0 ignored。测试profile完成耗时19m20s，实际测试0.01s。使用Rust1.97.1、既有F盘target、buildjobs1/testthreads1，没有修改依赖或工具版本。

日志 `task6-wrapper-all-features-prebuild-2026-10-08.log` SHA256：`037f3b1bf4126be6c3ee253105b49790dec9eb1ba7e54f0d939bfe345fb7acda`。机器summary为同名`.summary.json`。

16项包含真实标准外部租约完整委托、prepared资源materializer、普通关闭错误/重试、在途操作关闭及两类batch取消保护；HTTP SQL/JDBC Agent/libSQL的deterministic transport-owned/runtime-owned core trace不是实际远程数据库认证。本次没有设置正式验证器trace challenge，因此不能据此代替其挑战绑定证明。

原1090冻结Rust/Cargo/proto/SQL输入终态逐文件SHA一致。保留编译告警：Druid metadata proxy的unused unwrap_ref、Wrapper测试两处unused_mut及Windows linker消息。测试通过不等于clippy -D warnings通过。

## 完整 C2 后续执行

正式脚本help确认默认实际执行，只有`--no-execute`是静态模式。现已启动默认 `scripts/verify_task6_deleted_test_parity.py`，没有使用`--no-execute`，没有修改其1800秒子命令限制、测试断言或分母。

新日志：`task6-complete-c2-runtime4-2026-10-08.log`；新输入快照：`task6-complete-c2-runtime4-inputs-2026-10-08.json`。沿用逐项核验的原1090输入及同一Rust/Cargo/target环境。

运行句柄session56915；2026-10-08 01:56:40实际确认Python43372存活，尚未进入Cargo子命令。脚本会先执行完整源码/ledger核验且缓冲子命令输出；Tee日志在第一条输出前尚未创建，不以日志文件缺失或静默认定终态。继续观察同一句柄，待真实终态更新结果。

后续实际观察：01:58:54进入druid all-features pooled_statement_semantics目标，随后01:58:59进入rdbc_external_consumer目标。02:01:21确认Python43372、Cargo45584及实际消费者测试二进制34968存活。该目标仍在运行，整个C2没有终态；不从子进程切换推算聚合通过数，不因无输出重启。

原runtime3为1800秒超时NOT_RUN/ERROR，证据保留。独立Wrapper目标成功只完成先前昂贵编译路径；完整C2仍需逐一验证67个删除文件、1283个原测试身份及严格函数/参数化case执行标记。完整C1也尚未取得实际通过。

Task6仍FIX_REQUIRED；compiler coverage/API/SemVer/MSRV/features/完整workspace网络/最终审查与Task7八义务、全部18顶层任务继续开放。未安装工具、修改防火墙、切换分支、stage/commit/push或发布。

## runtime4 实际终态更正

session56915 最终 exit1。失败目标为 `druid-wrapper/sqlx_bb8_pool_test`，Cargo 编译已声明的 `druid-driver` 二进制时找不到 `src/bin/druid-driver.rs`，尚未进入该目标断言。此前 LIVE 观察是历史状态，独立 lib 16/0 不能覆盖这个二进制构建路径。

日志 SHA256：`4baa59916c32115eedeefcf310ac156ce09319c41f3dcbab2df0f6db32874bd2`；同名 `.summary.json` 保存失败阶段，aggregate pass count 为 null，不推算整体通过数。修复前再次逐项核验原1090输入，CHANGED_INPUTS=0。

后续恢复 Git `e897e77` 中原 Admin CLI 的完整命令实现到 Wrapper；只迁移 import ownership，并将源码路径改为 `src/bin/druid_driver.rs`。不删除二进制、不缩减 features 或验收分母。独立 CLI 合同新增测试的修复前命令实际 exit101，记录为编译 RED，不声称行为断言 RED；修复后及完整门禁结果另行记录。
