# DriverAction 标准归属恢复（2026-10-08）

DriverAction 的唯一真实 trait 已迁入 druid-rdbc/src/driver/driver_action.rs。Druid 历史模块与 sql 门面重导出同一 trait，DriverManager 调用策略未改。

## 实际合同与验证

新增 standard_driver_action_test.rs：标准路径的 Arc<dyn DriverAction> 无转换交给历史路径和产品门面，再注册到实际 DriverManager；重复注册保留第一次回调，成功注销通知一次，重复注销返回false且不再通知。

- RED 原命令实际 exit101/E0432，RDBC 标准公开路径缺失。
- GREEN 相同目标实际 exit0，1PASS/0FAIL。
- cargo fmt --all -- --check 实际 exit0。
- 完整 RDBC all-features 回归原句柄70019已实际exit0，477PASS/0FAIL。
- 新冻结1099源码+58控制输入。尚需终态复核、fresh canonical探针和完整workspace回归。

## 完成边界

此改动修复一个真实标准归属缺口，不统一 DriverManager、RDBC 与产品注册中心，不关闭P05。保留语义账本、oracle、原探针、Task7八项义务及全部18项顶层任务。Task6 FIX_REQUIRED，Task8 INCOMPLETE。

[机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-driver-action-validation2.summary.json)

完整workspace回归正在原句柄15014运行，尚无终态。

后续源码门禁：727生产文件的Physical/SPI与重复trait树检查通过；default normal核心依赖边界通过；final purity通过，STANDARD_FILES=50。仅源码组件，不构成public API或完整no-Physical CLI通过。

[源码门禁机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-driver-action-source-gates1.summary.json)

## 原始完整探针复验终态

捕获原workspace/all-features Cargo进程50152实际rustc子进程的--extern druid_rdbc rlib路径，保留完整compiler命令与SHA；未依赖目录最新时间猜测。用该精确产物复编译原始68Rust探针，所有探针源码SHA不变：42PASS/26FAIL，另1项非Rust目标未认证。DriverAction原失败trait-bound探针现在exit0。1099源码+58控制输入前后变化0。

原41/27/1、37/31/1诊断保留为历史；当前复验仅公开路径/trait bound，ENUM_KIND/MODULE import不认证精确类型形态或全部方法。原workspace runtime15014尚未终态。

[全量探针](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes3.summary.json)、[产物保留](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes3-retained.summary.json)。


2026-10-08 原完整workspace15014终态：收集器exit0、实际Cargo exit101，3869PASS/43FAIL/0ignored、259结果块；no-fail-fast已运行到后续目标及doctest结束。唯一失败目标grpc_network_e2e_test为3/43，1099源码+58控制输入变化0，取代先前LIVE观察；网络根因未作反事实认证。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。

[完整终态机器审计](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-driver-action-workspace-terminal-audit1.summary.json)。
