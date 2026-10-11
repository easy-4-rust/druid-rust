# P06 运行预算与故障验收

状态：**未关闭**。本记录只保存已实际运行的基线；缺少的指标和故障场景不计为通过。

## 2026-10-11 Pool 慢关闭组件核验

`PoolInner` 使用有界 `mpsc` 和关闭队列 permit；溢出时显式异步关闭受 `close_timeout` 约束，补池目标通过 `watch` 合并。`PoolState` 暴露队列深度、拒绝数、超时数、补池目标和 worker 终止状态。本机以 `--locked` 重跑 `pool_close_backpressure_test`（11/11）、`task6_pool_maintenance_semantics_test_parity`（14/14）和 `task6_recycle_complete_contract_test`（16/16），均退出 0。这证明已测的慢关闭、维护和回收路径；全仓回归及 P06 其他任务仍未关闭。

Metrics 的 `task_supervision_test`（2/2）、`shutdown_test`（8/8）、`queue_semantics_test`（4/4）、`config_test`（7/7）及 `aggregator_output_test`（17/17）同样以 `--locked` 退出 0，覆盖任务回收、panic 传播、关闭顺序、队列饱和和配置行为。仍需把这些局部证据与网络故障注入、全仓回归及运行预算合并验收。

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

上述预算只针对这台机器和这一负载。仍须在 Linux CI 上独立测量并确定稳定阈值；还须测量慢物理关闭的队列/超时、Metrics 饱和时的 dropped/coalesced、Admin 不可用与重启、重复 batch/序列缺口、过期条目、无效认证和待 ACK 时 SIGTERM。未取得这些数据前，P06 Task5 保持开放。
