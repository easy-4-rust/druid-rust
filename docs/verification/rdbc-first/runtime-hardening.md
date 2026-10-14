# P06 运行预算与故障验收

状态：**未关闭**。本记录只保存已实际运行的基线；缺少的指标和故障场景不计为通过。

2026-10-15 JDBC Agent 握手与协议错误脱敏：先运行新增 `malformedHandshakeAndJsonDoNotEchoUntrustedPayloads`，因不支持的 capability 名称出现在 JSON-RPC 错误响应中而失败（RED）。握手能力、协议版本错误改为固定诊断；无法解析请求时返回固定 `invalid JSON-RPC request`，日志只记请求 ID 和异常类型，不记解析异常内容。回归测试同时注入能力标记、畸形 JSON token 和非法 JSON-RPC 版本标记，并断言消息与完整错误对象都不含标记。`mvn -f agents/jdbc-agent/pom.xml -Dtest=AgentServerTest,XaAgentProtocolTest test` 8/8 通过，包含 PostgreSQL Testcontainers XA 协议测试。此处验证错误响应不回显；日志路径由固定异常类型格式确认，尚未用日志 appender 做端到端断言。P06 Task5 的其他故障和运行预算仍开放。

2026-10-15 Windows/Rust 1.97.1 Admin 内存预算复测：将原 Linux-only `/proc/self/status` 峰值 RSS 检查改为 Linux/macOS RSS 与 Windows Working Set 的跨平台物理内存采样，每 32 条成功写入采样一次并在结束时补采样；指标不可用会使测试失败。`cargo test -p druid-admin --test repository_capacity_test --locked` 10/10 通过；两次单项 `-- --nocapture` 输出均为 accepted=1044、retained_state_bytes=67,071,714，采样 Working Set 峰值为 75,739,136–76,046,336 字节，上限 134,217,728 字节。该值是采样峰值，不代表操作系统全程峰值。随后 `cargo test -p druid-admin --locked` 在 37 项 gRPC 网络用例通过后，有 13 项重连/ACK 用例超时；这是失败的本机全 crate 回归，不能计为通过。Linux/macOS 的当前变更 CI 证据待推送后记录；P06 Task5 仍开放。

2026-10-15 Windows/Rust 1.97.1 复测：Metrics 容量 1、20 个注册数据源的饱和关闭测试输出 coalesced=159、shutdown=124 µs，且关闭后 active_tasks=0、unflushed=0；Admin 两代会话 churn 写入 p50/p95/p99 为 107/175/178 µs，TTL 淘汰 64 条并保留新代 64 条。对应测试分别为 `saturated_runtime_shutdown_drains_and_joins_every_owned_task` 与 `expired_session_churn_reclaims_capacity_without_mixing_boot_identities`，均以 `--locked` 通过。这是各单次复测，不扩展成跨平台延迟承诺；Admin 长期 churn 的进程峰值 RSS 仍未测，Task 5 保持开放。当前 [CI run 38616750204](https://github.com/easy-4-rust/druid-rust/actions/runs/38616750204) 尚在执行，终态未计入本记录。

2026-10-13 SIGTERM 待 ACK 进程试验：关闭错误的 HTTP 状态先复现 400（RED），将 broker 的 `shutting down` 归入脱敏 503 后，目标单测 GREEN，严格 Clippy 通过。新增 Unix-only 真实子进程试验：认证的 gRPC 流已收到命令而未 ACK 时发送 SIGTERM，要求 HTTP 等待者在 2 秒内收到脱敏 503、进程在 4 秒内退出，均早于 5 秒关闭期限。[Linux Rust contracts](https://github.com/easy-4-rust/druid-rust/actions/runs/38397483551/job/115296951596) 日志明确显示 `sigterm_releases_pending_command_http_waiter_before_deadline ... ok`，同一 `admin_binary_test` 19/19 通过，已有的 held-connection SIGTERM 用例也通过。此结果只关闭待 ACK SIGTERM 故障场景，完整 P06 数值预算与其他故障注入仍开放。

2026-10-13 Admin 待 ACK 关闭：新增真实 Tonic 双向流测试，先确认运维命令已送达且客户端未 ACK，再取消服务关闭令牌。原实现中命令等待者超过 1 秒仍未结束（RED）；broker 与 IngestService 共享关闭令牌后，等待者在 1 秒内明确失败、关闭后的新命令被拒绝、受监管 stream 与服务任务均退出（GREEN）。生产构建顺序的令牌绑定单测通过；同一二进制的待 ACK 关闭单项、命令日志满额重连与半关闭结果重放两项回归通过；严格 Clippy 通过。本机完整 gRPC 网络套件在运行至后段时 38/49，后续连接出现 TCP 10060/5 秒超时；同一二进制复制到 D 盘新路径后，曾失败的半关闭重连单项与新关闭单项均通过，重新跑完整套件仍在后段复现超时。覆盖编译并发时再次单跑新用例也在建立 TCP 连接前发生 Windows 10060，未执行到待 ACK 断言；编译结束后，从 D 盘新路径独立运行待 ACK、生产构建顺序和两项重连回归，4/4 通过。故不能把本机整套记为通过，仍须以跨平台 CI 和独立的进程级 SIGTERM 待 ACK 试验关闭 Task5。

2026-10-13 Admin TTL/session churn：新增 `expired_session_churn_reclaims_capacity_without_mixing_boot_identities`，以 32 个 service、每 service 2 个 datasource、TTL 1 秒填满仓库，等待 1.1 秒后注入新一代 boot identity。旧代 64 个条目均不可见，新代仍为 64 个，TTL 淘汰计数为 64，容量拒绝计数为 0；本机首次两轮写入 p50/p95/p99 分别为 79/240/333 µs 与 71/140/155 µs，同一测试可执行文件随后重复 10/10 通过，10 轮 p99 为 141–217 µs。`repository_capacity_test` 5/5、`ingest_test` 11/11、`prometheus_test` 8/8 通过。这是短周期内存仓库基线，尚未测得进程峰值 RSS、长期 churn 与 gRPC 端到端延迟，不能据此关闭 Admin 数值预算。

2026-10-13 Admin 全局计数预算：默认额外限制整个最新状态仓库为 4,096 个 datasource 和 16,384 条 SQL identity，保留原五参数配置构造函数并提供可收紧的 `with_global_limits`。新增测试验证全局 datasource/SQL 预算超限时 incremental batch 与权威快照均原子拒绝，既有 identity 覆盖不重复占位；首次本机 `repository_capacity_test` 7/7、`ingest_test` 11/11、`prometheus_test` 8/8 通过。投影字节预算的后续验证见下文；计数上限本身不能替代 RSS 验收。

2026-10-13 Admin TTL HTTP 可见性：`expired_boot_session_disappears_from_rest_and_prometheus` 经真实 Axum 路由验证，TTL 前 `/druid/api/datasources` 有 1 项、单项详情返回 200、`/metrics` 的 datasource gauge 为 1；过期后列表为空、详情 404、gauge 为 0，客户端时间戳故意设为最大值也无法延长服务端 TTL。本机 `prometheus_test` 9/9 通过，该单项随后重复 10/10 通过；这些短周期验证不代替长期 churn 或进程内存预算。

2026-10-13 Admin 投影字节预算：新增默认 64 MiB 的全仓库 datasource/SQL/wall 序列化投影限额，在写锁外使用有界计数写入器计量输入，不建立完整 JSON 副本。`total_state_bytes_budget_rejects_large_sql_batches_and_reclaims_replaced_bytes` 验证单条及批次超限拒绝、权威快照原子性、覆盖旧值后预算释放；`/metrics` 暴露当前投影字节和配置上限，TTL 后当前值归零。锁外计量后的目标回归 `ingest_test` 11/11、`prometheus_test` 9/9、`repository_capacity_test` 8/8、`servlet_test` 29/29、库内测试 5/5 通过，严格 Clippy 通过。短周期 churn 再重复 10/10 通过，写入 p50 为 107–112 µs、p99 为 187–249 µs，较加入字节计量前的 70–72 µs p50 增加；尚未接受跨平台延迟阈值。该计量不包含 HashMap、对象头和服务身份键的内存开销，不能当成 RSS 上限；长时间 churn 与实际峰值 RSS 继续待验收。

2026-10-13 本机 TLS 进程测试诊断：`admin_binary_test` 14/16 通过，两项 TLS 启动测试在本机 Windows 连接超时；失败时子进程仍存活且 `0.0.0.0` 端口处于 Listen，另一个本机进程的原生 TCP 连接在 500 ms 内仍无法建立。PowerShell 自建 loopback/Any 监听则能连接；同一源码的 [CI run 38378806395](https://github.com/easy-4-rust/druid-rust/actions/runs/38378806395) Windows Rust contracts 作业通过。以上只支持本机进程网络层异常的定位，不把本机两项失败记为通过，也不替代新提交的跨平台验证。

2026-10-13 Metrics 饱和关闭组合验证：当前 Windows/Rust 1.97.1 的真实 runtime 以队列容量 1、20 个保持注册的数据源、1 ms 采样周期运行至少 100 ms，新增 `saturated_runtime_shutdown_drains_and_joins_every_owned_task` 测试要求先发生合并，再检查关闭后受监管任务、未刷新工作和注册数均为 0。完整 `queue_semantics_test` 5/5 PASS；直接运行同一测试可执行文件 20/20 PASS，合并次数每次 159–239，关闭耗时 123–333 µs（20 样本最近秩 p99=333 µs），轮询所得进程峰值工作集最高 5,420 KiB。这是本机参考基线，尚未给出跨平台验收阈值；Admin 数值预算、SIGTERM 待 ACK 及完整故障矩阵继续开放。

2026-10-13 [CI run 38331373881](https://github.com/easy-4-rust/druid-rust/actions/runs/38331373881) 的 PostgreSQL/MySQL 均在重启 hook 与建池后借用连接超时；[CI run 38332281893](https://github.com/easy-4-rust/druid-rust/actions/runs/38332281893) 的原生直连探针进一步确认，容器内部报告 healthy 后，旧随机映射宿主机端口立即返回 `Connection refused (os error 111)`。改为固定映射端口并同时检查宿主机连通性后，[CI run 38332794498](https://github.com/easy-4-rust/druid-rust/actions/runs/38332794498) 的 PostgreSQL 16.15 与 MySQL 8.4.11 真实产品合同均成功，上传的证据 JSON 各含 `database-restart` 等 13 项合同、对应源码 SHA `a656764fbb04603accedb3a28836f1b824386b34`。这只关闭两项实库重启故障的当前验证缺口；P06 数值故障预算及整轮发布门禁仍未关闭。

2026-10-13 对 [CI run 38232002006](https://github.com/easy-4-rust/druid-rust/actions/runs/38232002006) 的终态审计：38 个作业成功、SemVer 失败；PostgreSQL、MySQL 和 x86_64 macOS/Windows 默认工具链 DuckDB 契约在约 6 小时后取消，均不计为实库通过。DuckDB 两个平台最后停在超时语句关闭后、同租约 `ping` 前；本机相同契约在旧实现第 11/25 次复现 20 秒卡死，延长后台中断至 10 秒仍卡死。改为取消回调立即调用原生 interrupt、保留后台重试后，当前 Windows `1.97.1` 同一实库契约连续 100/100 次在单次 20 秒期限内退出 0。[新 CI run 38327320344](https://github.com/easy-4-rust/druid-rust/actions/runs/38327320344) 的 DuckDB 五目标双工具链契约 10/10 通过且证据聚合成功。PostgreSQL/MySQL 原运行停在重启阶段；重启 hook 现输出阶段、限制 `docker restart` 为 90 秒，实库 CI 作业设 45 分钟硬期限；该新运行的两项均在服务恢复 healthy 后、重建连接段挂起并被作业上限取消，没有生成认证证据。下一轮已为 hook 完成、建池、借用和 ping 增加阶段标记与 30 秒局部截止时间，整条测试限 10 分钟。Task5 保持开放。

2026-10-13 本机 `cargo test --workspace --all-features --locked -j 1` 编译完成后在 Admin gRPC 网络套件 42/48 失败，独立同配置重跑仍为相同六项；默认 feature 重跑有大量初连超时，不能算本机全仓通过。相同源码的上述 CI Windows `Rust contracts` 作业中该套件 48/48 通过，故本机回环异常与代码缺陷尚需进一步区分。

2026-10-12 当前本地 `main`（`a312854`）整 crate 复测：`cargo test -p druid-admin --locked --quiet` 退出 0，共 139/139 通过，其中网络测试结果块为 48/48。此结果确认当前 Windows 测试进程中的 Admin 行为回归；Metrics/Admin 的独立数值运行预算和跨平台故障验收仍未完成。

2026-10-12 `9ad07a4` Windows 当前源码复测：`cargo bench -p druid-bench --locked --bench runtime -- --noplot` 退出 0；直接对同一可执行文件运行 `--bench --noplot` 并监测峰值工作集，100 样本的 p50/p95/p99 为 123.599/124.751/125.755 ms，峰值工作集 20.92 MiB，等待队列峰值 56、活动连接峰值 8、关闭 7 ms，均在下方本机参考预算内。此测量只覆盖 Pool burst，不扩大为 Metrics/Admin 故障预算。

2026-10-12 `394f116` 复测：Admin `servlet_test` 28/28、`admin_binary_test` 16/16、命令 broker 错误脱敏单测 1/1 通过。`grpc_network_e2e_test` 在原路径首次初连即超时；同字节二进制复制到新路径后整套 48 项运行至 37/48，后 11 项出现 TCP `10060`/5 秒初连超时。将这 11 项分别复制为新文件名并独立运行后全部通过。该证据支持本机按可执行文件路径/运行时长触发的回环异常，但**不构成整套网络测试通过**；仍以 CI 与稳定环境的完整运行作关闭依据。

2026-10-12 当前 `d528486` Windows 复测：Pool 关闭/维护/回收分别 11/11、14/14、16/16；Metrics Supervisor/shutdown 分别 2/2、8/8；Admin binary（含真实 TLS/认证）16/16。`grpc_network_e2e_test` 常规路径 45/48，三项失败；单独重跑时普通生成的 Tonic 客户端也在初连收到 TCP `10060`。临时加入同一测试二进制的纯 `std::net`/Tokio 回环探针均超时，而独立 Rust/Python 探针成功；复制该测试二进制到另一位置后纯 TCP 探针曾通过，完整顺序运行又在后段出现回环超时（39/50，包含两项临时探针）。临时探针已从源码移除且未提交。此现象定位到本机测试进程的回环边界，尚不能据此判断协议缺陷或宣称跨平台网络门禁通过；Linux/Windows CI 结果与该本地异常需分别核对。

2026-10-12 本机复测：Pool `pool_close_backpressure_test` 11/11；Metrics `task_supervision_test`、`shutdown_test`、`queue_semantics_test` 合计 14/14；Admin `grpc_network_e2e_test` 与 `repository_capacity_test` 合计 52/52，`admin_binary_test` 16/16，均使用 `--locked` 并退出 0。网络用例覆盖 Admin 重启、重复/跳号批次、服务不可用、命令认证及关闭；仍缺这些故障场景各自的数值运行预算与跨平台验收，不据此关闭 Task5。

2026-10-12 修正 Metrics 饱和测试夹具：原用例立即释放数据源和注册句柄，不能证明真实 sampler 的容量 1 队列发生合并。加入 `coalesced_total > 0` 后测试先失败；保持 20 个源及句柄存活后，确认所有源被采样、发生合并，`queue_semantics_test` 4/4 与相关 Supervisor/shutdown 合计 14/14 通过；`cargo test -p druid-metrics --locked` 整个 crate 退出 0。仍需独立的吞吐、延迟与关闭预算。

2026-10-12 发现队列饱和同时错误增加 `snapshot_busy_total`：真实采样器测试新断言先以 608 对 0 失败；修复后只增加 `coalesced_total`，实际 Busy 数据源仍只增加 `snapshot_busy_total`。`queue_semantics_test` 4/4、Metrics 默认 feature 128/128、`--all-features` 129/129 通过。此项修正告警归因，不代替数值预算。

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

提交 `e1e6d22` 的[主线复测](https://github.com/easy-4-rust/druid-rust/actions/runs/38165824500)通过：100 样本 p50/p95/p99 为 24.758/25.089/25.122 ms，峰值 RSS 19,120 KiB，等待队列峰值 56、活动连接峰值 8、关闭 2 ms；上传 JSON 的五项预算均无超限。此提交包含 Wrapper 取消关闭修复；测量仍只覆盖 Pool burst，不能替代 Metrics/Admin 故障预算。
