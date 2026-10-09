# Task6 全功能回归终态与质量门禁后续（2026-10-08）

## 已取得的终态

原 workspace/all-features session52736 已 exit101。命令为 `cargo test --workspace --all-features --locked --offline -- --show-output`；164 个已完成结果块累计2886 PASS/8 FAIL/0 ignored，Admin binary 14/0，gRPC network 38/8。fail-fast停止剩余目标，不能声明全仓通过。原1092源码与58控制输入终态一致。本轮再次逐项复核亦一致，随后才修改两个测试import。

原日志与summary位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-workspace-all-features-runtime1-2026-10-08.*`，日志SHA256 `ead36cb62a1110ef1ea461870b7e1fe93cad1cc59363872e2d5b9b68fd558dd6`。完整C1/C2既有PASS保留原冻结范围，不能自动扩大到后来修改的输入。

## 网络环境证据

CodeGraph确认最先失败的resync测试经实际probe server、GrpcMetricsClient、send_batch和shutdown执行；失败是shutdown未清空1项，之后七项连接/建连超时。只读Get-NetFirewallApplicationFilter/Get-NetFirewallRule发现实际全features程序 `F:/codex_build_cache/druid_rust/task6_rust_1971_20261007/debug/deps/grpc_network_e2e_test-20b9d4a616891dd0.exe` 对应两条启用的Public入站Block规则；当前WLAN/以太网/Meta配置为Public。记录为 `task6-grpc-firewall-readonly-2026-10-08.json`。

没有更改安全设置、程序路径、测试超时或断言。该证据支持环境阻断候选，但不证明8项失败同源；尤其首项shutdown语义仍需独立对照。不得将旧hash Allow规则当成当前程序的授权或绕过路径。

## 本轮质量修复

删除 `crates/druid-admin/tests/admin_binary_test.rs` 未使用的AsyncWriteExt import；删除 `crates/druid/tests/task6_strong_read_filter_contract_test.rs` 未使用的Pool import。没有生产逻辑或测试断言修改。其余unused_mut与私有metadata unwrap_ref警告尚未处理：需保持原迁移证据及语义，不能用批量allow或删除行为掩盖。

新冻结记录 `task6-unused-import-inputs-2026-10-08.json`，1092源码/58控制输入相对上一冻结仅上述两项源码SHA变化。

## 在途验证（尚无终态）

- Cargo check，session2128 / PID45484：`check -p druid-admin --all-features --test admin_binary_test -p druid --test task6_strong_read_filter_contract_test --locked --offline`，Rust1.97.1、jobs1、原target缓存；实际DuckDB build-script PID50024运行。日志 `task6-unused-import-check-2026-10-08.log`。
- 源码迁移核验，session34393 / Python45644，子C2 Python20976：`verify_task6_method_parity.py --no-execute`；日志 `task6-unused-import-source-parity-2026-10-08.log`。即使通过也只属于source检查，不代替runtime C1/C2。

不得仅由进程存在或本报告判断PASS。继续观察原句柄，禁止因输出暂空重新启动。Task6仍FIX_REQUIRED；Task7八义务与全部18顶层任务保持OPEN。

## 源码终态与标准合同文档

原session34393实际exit0：479方法映射、442唯一选择器、executed=no；修改后1092源码/58控制输入变化0。仅source检查通过，不是新的runtime C1/C2认证。Cargo check原session2128仍在DuckDB构建，尚无终态。

已按ADR-RDBC-006补齐 `docs/rdbc/standard-contract-v1.md`，涵盖唯一接口、所有权、NULL/重载、生成键、结果序列、metadata/Wrapper、错误、资源/capability/streaming及完成门禁；明确Task7和发布未验收。相对链接、闭合代码块和空白检查通过。该文档是既有规格的合同说明，不创建新规格事实源，不关闭Task8。

## 集成作者指南交付

已根据实际factory、外部租约及SQLite共享合同，补齐 `docs/rdbc/integration-author-guide.md`。指南覆盖唯一连接所有者、owned actor/statement/resultset、重载/NULL/生成键、资源取消、真streaming、feature/registry现状、实际共享合同范围和证据账本。命令仅列现有目标，不声称重新运行。两份合同文档的相对链接、闭合代码块、空白以及命名测试目标存在性验证通过。1092源码/58控制输入仍变化0；check原session2128继续运行。文档产物完成不等于Task8完成，Task6FIX_REQUIRED/18OPEN。

2026-10-08 编译终态：原session2128 exit0/13m13s，两个受影响测试目标all-features编译通过；未执行runtime，private metadata unwrap_ref警告保留。终态1092源码/58控制输入变化0。source479/442已PASS/executed=no，整体Task6与18项仍OPEN。

2026-10-08 正式质量门禁推进：workspace fmt --all -- --check实际exit0，机器summary已保存。正式check --workspace --all-targets --all-features --locked --offline原session80815运行，冻结1092源码/58控制输入继承两项import修复后的记录。原两目标check2128已exit0。完整workspace check尚无终态；不关闭Task6/18OPEN。

2026-10-08 全编译门禁终态：原session80815实际exit0/4m22s，check --workspace --all-targets --all-features --locked --offline通过；1092源码/58控制输入终态变化0。五处警告保留，不是-Dwarnings、runtime、MSRV、API或发布证明。workspace fmt此前实际exit0。Task6FIX_REQUIRED/18OPEN。
