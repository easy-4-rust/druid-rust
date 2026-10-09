# 当前工具链可用性核验（2026-10-07）

这是只读的本地可执行文件/版本核验，不是编译或兼容性认证。Task6–8/P05–P07完整18任务保持开放。

规格及根 `rust-toolchain.toml` 要求默认1.97.1，MSRV为1.95。本次 `rustup toolchain list` 实际列出Windows stable、1.75.0、1.89.0、1.97.1、1.99.0，以及macOS aarch64的nightly-2026-08-07；跨平台nightly存在不证明Windows能执行它。

实际执行Windows `1.97.1-x86_64-pc-windows-msvc/bin/rustc.exe --version` 返回 `rustc 1.97.1 (8bab26f4f 2026-07-14)`；已安装且可执行，无需为该默认工具链安装。Windows stable实际返回 `rustc 1.99.0 (b940084d7 2026-09-28)`。当前完整Object workspace session6505显式使用stable，属于1.99.0范围；它即使通过也不能关闭1.97.1/MSRV门禁。后续默认工具链验证使用已有1.97.1。

本地Windows1.95.0 rustc预期路径不存在，toolchain list也没有1.95项。MSRV仍NOT_VERIFIED，不用1.99结果替代。`Get-Command cargo-public-api,cargo-semver-checks` 无结果；stable Cargo实际 `--list` 中也没有public-api/semver命令。仅能证明本次CLI发现范围缺失，不声称所有磁盘路径均不存在。按P04 Task8 Step2工具缺失只暂停该门禁，不缩小完整目标，不静默安装。

同一完整回归session6505经句柄及actual cargo69032/编译子进程核实LIVE：本轮由druid lib test推进到rustc63680 `task6_log_filter_full_test_parity_part_01`。未重启、未切换当前运行工具链、未修改Rust/Cargo冻结输入；完整终态尚未取得。

## 接续默认工具链检查

同一1.99.0完整回归已完成编译（实际25m55s）并进入测试阶段，尚无终态。为推进独立默认工具链证据，在冻结源码上启动实际1.97.1 `cargo check --workspace --all-targets --locked --offline -j1`，session11062/actual cargo71308确认运行。采用独立缓存 `F:/codex_build_cache/druid_rust/task6_rust_1971_20261007`，没有复用或改变6505的target目录；不修改锁文件、源码、工具链配置。

日志为 `task6-rust-1971-workspace-all-targets-check-2026-10-07.log`，独立990项输入freeze为 `task6-rust-1971-workspace-inputs-2026-10-07.json`。目前已推进druid-rdbc及依赖检查，尚无终态，不声明编译通过，更不声明运行测试、全feature或MSRV1.95通过。原6505的990项输入fresh全部相同。

完整标量候选接入脚本已增加这项检查的真实通过summary和日志哈希前置条件，两个运行句柄取得终态前不接入Rust候选。此前准备阶段只存在一个Cargo的历史描述不代表现在；当前明确是两个独立缓存、同一冻结源码的已授权检查。
