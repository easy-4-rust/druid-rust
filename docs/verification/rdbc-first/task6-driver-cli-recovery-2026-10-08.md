# Task6 驱动 CLI 构建恢复（2026-10-08）

## 范围与事实源

继续现有 Brownfield Superpowers 计划 P04 Task6；规格事实源不变，不创建新的规格。CodeGraph 确认受管安装、诊断、证据聚合逻辑已迁入 Wrapper，但没有对应 CLI 入口。Git 历史 `e897e77` 提供原完整 250 行 Admin CLI。P05 要求 `druid-driver` 由 Wrapper 持有；本次只恢复阻塞 Task6 构建的入口，不关闭 P05。

## 原始失败证据

完整 C2 runtime4/session56915 已 exit1，`sqlx_bb8_pool_test` 构建无法读取 manifest 声明的 `src/bin/druid-driver.rs`。日志 SHA256 为 `4baa59916c32115eedeefcf310ac156ce09319c41f3dcbab2df0f6db32874bd2`。修复前原1090输入逐文件 SHA 一致，同名机器 summary 保存真实失败阶段；不推算完整 C2 的聚合通过数。

新增 CLI 合同测试后实际命令：

```text
cargo test --offline --locked -p druid-wrapper --all-features --test driver_cli_test -- --show-output
```

exit101，仍因缺少二进制源文件失败。日志为 `task6-driver-cli-red-2026-10-08.log`；这是编译 RED，不声称测试断言 RED。

## 恢复实现

- 原 CLI 全部命令逻辑恢复到 `crates/druid-wrapper/src/bin/druid_driver.rs`：catalog/list、Agent/JRE/单 JAR/bundle/URL 安装、verify、activate、remove、doctor 与 aggregate-evidence。
- 原 `druid_admin::driver` import 改为实际所有者 `druid_wrapper::driver_admin`；其他命令语义保留。
- manifest 仅将 bin path 改为 snake_case；保留 `managed-driver-install` 条件及所有其他已有用户修改。
- `git check-ignore -v` 发现 `.gitignore:14:bin/` 隐藏恢复入口；新增只针对 Wrapper 源目录与该文件的例外，其他 bin 输出仍忽略。Git 历史显示迁移提交删除 Admin 入口但未纳入 Wrapper 入口，忽略规则是相关证据，不能仅据此断言当时丢失的唯一原因。
- 新增四项实际进程级合同测试：完整帮助、80产品及真实支持数、缺参/未知命令错误、非法证据 runtime mode 拒绝。
- 没有通过删除 bin、缩小 features、降低断言或修改超时绕过原失败。

## 实际验证

```text
cargo test --offline --locked -p druid-wrapper --all-features --test driver_cli_test --test sqlx_bb8_pool_test -- --show-output
```

session71296 终态 exit0：CLI 4 PASS/0 FAIL，原 bb8 目标5 PASS/0 FAIL。日志 `task6-driver-cli-green-2026-10-08.log` SHA256 为 `968d3693d1ef60592339b4efe168343c0c794bb15bad60ea55a4ed3ec41734f6`。Rust1.97.1、既有 F 盘 target、单 build job/test thread、offline/locked 环境不变。告警保留，不等于 clippy 门禁通过。

修复后快照保留原1090输入范围（包括原快照的缓存输入），只更新 Wrapper manifest SHA，并新增入口和测试两个输入，共1092项；验证终态逐项无变化。

## 完整门禁继续执行

已启动默认实际完整 C2 runtime5/session55218，日志 `task6-complete-c2-runtime5-2026-10-08.log`，快照 `task6-complete-c2-runtime5-inputs-2026-10-08.json`。未使用 `--no-execute`，固定67文件/1283原测试身份、479方法映射以及严格函数/参数化执行标记要求不变。等待真实终态，不从局部9/0推定完整门禁通过。

Task6 仍 FIX_REQUIRED；Task7八义务、Task8及P05/P06/P07共18顶层任务保持开放。没有安装工具、更改防火墙、创建/切换分支、stage/commit/push或发布。

## 后续实际观察

runtime5 原 session55218 仍存活，Python41348 为正式验证器；02:26:16观察到 `druid/task6_pool_coverage_parity` Cargo41568，随后切换到 Cargo37948 与实际测试进程42064（02:26:41/42启动）。没有从子进程切换推算整体通过数。原1092冻结输入逐文件重新检查，CHANGED_INPUTS=0。

全workspace显式Cargo目标path存在性审计 exit0：当前显式目标只有 Wrapper `druid-driver`，恢复路径存在，Git status 已可见为未跟踪源码。审计 JSON 为 `task6-explicit-target-path-audit-2026-10-08.json`，记录 `.gitignore` SHA。该检查不覆盖自动发现目标的运行，也不替代 all-targets build 或完整 C2。
