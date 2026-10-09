# Task6 导航修复全仓回归与网络失败（2026-10-07）

原全workspace session23625已取得真实终态exit101：253结果块、3795PASS、11FAIL、0ignored，包含2个通过doctest。默认Rust1.97.1；命令保持workspace/locked/offline/no-fail-fast/-j1/test-threads=1。启动时1023项Rust/Cargo/proto/SQL输入SHA在终态全部一致，没有在运行期间改动构建输入。

日志为 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-navigation-filter-workspace-2026-10-07.log`；SHA8b1bac78a28be641f32854655ee1e1c3f7365320db4ca9fe09d9a413f5b2b3e4。对应summary保存真实终态、范围、总数与全部失败测试名称。此次结果替代同一句柄的LIVE说明，不能被较早源码3787/0结果替代。

失败目标一：Admin admin_binary_test，13PASS/1FAIL。remote_http_and_grpc_use_tls_and_require_authentication 在HTTP TLS启动连接处失败，Windows TCP10060连接超时；测试丢弃子进程stderr，当前证据不足以判断监听、握手或外部环境的具体原因。

失败目标二：Admin grpc_network_e2e_test，36PASS/10FAIL。reset_result_survives_real_tonic_half_close_and_reconnect_without_reexecution 在command reconnect等待处Elapsed，其HTTP任务报告live stream出现超时。后续多个场景在GrpcTransportSetupTimeout5000或TCP10060处失败，另有shutdown超时观察失败。不能把这些失败自动归因于导航Filter改动，也不能自动认定偶发网络或冷启动资源争用。

只读CodeGraph检查已读取两个失败测试及关键fixture调用。独立Python回环TCP12样本成功，仅证明该进程当时的普通loopback TCP可连；不覆盖同一Rust进程、TLS、HTTP/2、gRPC、重连或服务生命周期，不能作为产品通过证据。

新独立复现session77706已经启动，默认Rust1.97.1，执行两个完整失败目标，保持locked/offline/no-fail-fast/-j1/test-threads=1。日志task6-navigation-admin-network-reproduction-2026-10-07.log。package范围改变导致依赖feature统一范围不同，正在编译，未有终态。没有修改源码、超时、断言或公共能力；采用systematic-debugging先定位根因。复现通过仍不能单独认证网络稳定或把原全仓失败写成通过。

Object更新完整候选继续NOT_COMPILED/NOT_RUN/NOT_BOUND，保护式接入脚本要求真实全仓通过终态，未绕过前置条件。正式1130EXACT/153PARAMETERIZED、sourcefloor1267PASS/16FAIL不变；Task6 FIX_REQUIRED，Task7八项及全部18顶层任务、API/compiler/SemVer/MSRV/features/最终审查仍开放。

## 新鲜启动与系统策略证据

受控现有Admin二进制启动探针已执行，二进制SHA74a09cdc67fe66f64ae5efaa00b0b22b65d89031d37521884d1af98c46f2f8fa。日志输出starting druid-admin，进程存活，3秒内7次请求均连接超时。该探针按原真实证书、TLS和Basic配置启动，结束后仅清理自己创建的进程；没有修改系统或产品配置。

进一步三组分阶段探针：wildcard TLS、loopback TLS、loopback plain。每组的Windows TCP快照都显示该进程HTTP与gRPC端口处于Listen；客户端都在tcp_connect阶段超时，未进入TLS或HTTP阶段。这把探针故障边界定位在已有监听端口的TCP可达性，不能称作证书、握手或认证失败。证据保存task6-navigation-admin-tls-startup-probe-2026-10-07.json与task6-navigation-admin-stage-probes-2026-10-07.json及相应服务日志。

只读Windows ActiveStore查询发现：当前网络为Public；F:/codex_build_cache/druid_rust/task6_rust_1971_20261007/debug/druid-admin.exe有两条Enabled/Public/Inbound/Block规则（TCP、UDP），同缓存下grpc_network_e2e_test-c3d8212528733332.exe也有两条对应Block规则。原全仓日志确认实际gRPC测试二进制正是grpc_network_e2e_test-c3d8212528733332.exe，匹配上述明确Block规则；默认入站策略为Block。旧缓存与旧target路径中的Allow规则不覆盖当前路径，不能通过更换路径绕过阻止策略。完整限定路径规则/地址/协议只读快照已保存task6-navigation-firewall-readonly-snapshot-2026-10-07.json。

显式阻止规则与已监听但TCP超时直接吻合，是当前环境门禁的明确证据；尚未通过授权调整后的相同测试对照证明这些规则解释全部11项失败。当前用户令牌非管理员。已异步请求用户是否允许备份相关规则并仅为本次Admin/gRPC测试程序配置127.0.0.1/::1入站许可；答复待定。没有删除、禁用规则，没有关闭防火墙，没有提升权限，没有开放远程地址；也没有弱化测试或改写产品代码。

独立复现77706/cargo31364实际仍运行并推进至druid-admin编译，无终态。继续同一句柄，不因等待或策略发现重启。全部构建输入继续冻结；审批待定不等于已授权，不执行依赖审批的操作。

## 独立复现终态与独立合同继续实施

77706已取得真实终态exit101：两个完整目标49PASS/11FAIL/0ignored，TLS13/1、gRPC36/10，全部失败名称与原全仓一致；源码仍为原1023输入且SHA全部不变。日志SHA66e388b2b39f8882b6c9ad7b1a86917e68c26e66688cfb8d00108a74ae23e891，summary保存完整范围与失败列表。package feature统一范围不同，实际gRPC二进制为38a461ffd3bd2d97；只读ActiveStore查询也发现其TCP/UDP Enabled/Public/Inbound/Block规则。系统调整授权仍待定，无权限提升或防火墙更改。

本地候选接入脚本原本要求全仓通过，是实施保护条件而不是允许把网络失败缩减掉的验收豁免。现明确区分独立实施与最终验收：在原全仓、完整失败目标都已终态、全部失败严格限定相同11项、两个日志SHA有效、原1023输入未变、无live Cargo和无覆盖前提下，允许继续独立Object更新合同实施；原全仓/网络/最终验收仍FAIL/OPEN。没有修改任何正式验证器断言或门禁标准。

完整Object更新主测试和两个单对象夹具已经实际接入并格式检查通过；只改变diagnostic_tests.rs并新增两个fixture文件，没有生产路由修改。默认1.97.1完整Druid lib测试74226已启动，当前编译中，无通过声明。原1023输入加两个新夹具共1025项SHA已冻结，运行期间不再改构建输入。候选仍NOT_BOUND，正式1130/153、1267/16保持不变；整个目标及18顶层任务保持开放。
