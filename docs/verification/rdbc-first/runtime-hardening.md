# P06 运行预算与故障验收

状态：**未关闭**。本记录只保存已实际运行的基线；缺少的指标和故障场景不计为通过。

2026-10-12 本机复测：Pool `pool_close_backpressure_test` 11/11；Metrics `task_supervision_test`、`shutdown_test`、`queue_semantics_test` 合计 14/14；Admin `grpc_network_e2e_test` 与 `repository_capacity_test` 合计 52/52，`admin_binary_test` 16/16，均使用 `--locked` 并退出 0。网络用例覆盖 Admin 重启、重复/跳号批次、服务不可用、命令认证及关闭；仍缺这些故障场景各自的数值运行预算与跨平台验收，不据此关闭 Task5。

JDBC Agent 故障注入：`cargo test -p druid-wrapper --features jdbc-agent --lib --quiet` 本机 21/21 通过。沉默子进程启动按调用者配置的 75 ms 期限失败；不同身份的启动互不阻塞，同一身份的等待也遵守各自期限。超过 1 KiB 且无换行的帧在 EOF 前被拒绝，非法 ready/handshake 不回显注入的凭据标记。此项只覆盖 Agent 协议边界，不代替下表的跨平台运行预算或真实数据库验收。

## 2026-10-11 Pool 慢关闭组件核验

`PoolInner` 使用有界 `mpsc` 和关闭队列 permit；溢出时显式异步关闭受 `close_timeout` 约束，补池目标通过 `watch` 合并。`PoolState` 暴露队列深度、拒绝数、超时数、补池目标和 worker 终止状态。本机以 `--locked` 重跑 `pool_close_backpressure_test`（11/11）、`task6_pool_maintenance_semantics_test_parity`（14/14）和 `task6_recycle_complete_contract_test`（16/16），均退出 0。这证明已测的慢关闭、维护和回收路径；全仓回归及 P06 其他任务仍未关闭。

Metrics 的 `task_supervision_test`（2/2）、`shutdown_test`（8/8）、`queue_semantics_test`（4/4）、`config_test`（7/7）及 `aggregator_output_test`（17/17）同样以 `--locked` 退出 0，覆盖任务回收、panic 传播、关闭顺序、队列饱和和配置行为。仍需把这些局部证据与网络故障注入、全仓回归及运行预算合并验收。

Admin 的 `repository_capacity_test`（2/2）与 `ingest_test`（11/11）以 `--locked` 退出 0，覆盖复合会话标识、容量拒绝、TTL 和批次原子性；同一源码的 GitHub Windows `Rust contracts` 作业还运行了 Prometheus 与 gRPC 网络测试并成功。此处只确认对应组件，不把这些局部结果扩展为 P06 全部故障和安全验收。

## 2026-10-11 本机基线

- 源码：`6bee28d` 加上 `runtime.rs` 等待队列采样；Rust `1.97.1`；Windows 11 `10.0.26200`；i7-13700K（24 逻辑处理器）。
- 命令：`cargo bench -p druid-bench --locked --bench runtime -- --noplot`。随后直接运行同一 benchmark 可执行文件并监测进程峰值工作集；Criterion 取 100 个样本，按每样本 `times / iters` 的最近秩计算分位数。
- 场景：SQLite 池 `maxActive=8`，每轮 64 个并发借用，连接持有 2 ms。

| 指标 | 实测 | 本机参考预算（暂不作为跨平台发布门禁） |
| --- | ---: | ---: |
| 每轮完成时间 p50 / p95 / p99 | 123.895 / 125.332 / 125.970 ms | p99 ≤ 150 ms |
| 活动连接峰值 / 等待队列峰值 | 8 / 56 | 等待队列 ≤ 56 |
| benchmark 进程峰值工作集 | 16.68 MiB | ≤ 32 MiB |
| 池关闭耗时 | 3 ms | ≤ 250 ms |

上述 Windows 参考预算只针对这台机器和这一负载。

## 2026-10-11 Linux CI 基线与门禁

相同提交 `dcf1841` 在 Ubuntu 24.04 runner 的三次独立运行均成功：[第 1 次](https://github.com/easy-4-rust/druid-rust/actions/runs/38128370325)、[第 2 次](https://github.com/easy-4-rust/druid-rust/actions/runs/38128589274)、[第 3 次](https://github.com/easy-4-rust/druid-rust/actions/runs/38128589449)。每次均有 100 个 Criterion 样本和可下载的 JSON、原始日志及样本；等待次数为 18,638–24,338，确认负载确实发生争用。

| 指标 | 三次实测 | 后续 CI 上限 |
| --- | ---: | ---: |
| 每轮完成时间 p99 | 25.293 / 25.447 / 25.495 ms | 100 ms |
| 峰值 RSS | 18,464 / 18,840 / 18,620 KiB | 65,536 KiB |
| 等待队列峰值 / 活动连接峰值 | 56 / 8（每次） | 56 / 8 |
| 池关闭耗时 | 2 / 1 / 2 ms | 250 ms |

门禁脚本会先保存测量 JSON，再对超预算指标逐项报错。[预算门禁首次运行](https://github.com/easy-4-rust/druid-rust/actions/runs/38129014068)在提交 `ab18389` 上成功；上传的 JSON 记录 p99 25.486 ms、峰值 RSS 18,848 KiB、关闭 1 ms、全部五项上限及零超限项。边界等于阈值与五项同时超限的单元测试均已通过。慢物理关闭的队列/超时、Metrics 饱和时的 dropped/coalesced、Admin 不可用与重启、重复 batch/序列缺口、过期条目、无效认证和待 ACK 时 SIGTERM 仍缺运行预算。P06 Task5 保持开放。

当前提交 `bb1bea1` 的[手动 CI 复测](https://github.com/easy-4-rust/druid-rust/actions/runs/38134374659)通过：进程级故障注入验证超预算返回非零并保留 JSON；真实基准 100 样本测得 p50/p95/p99 为 24.758/25.112/25.163 ms，峰值 RSS 18,764 KiB，等待队列峰值 56，关闭 1 ms，五项预算零超限。该故障注入证明检查进程的失败传播；尚未通过故意让整个 GitHub 作业失败来验证工作流终态。
