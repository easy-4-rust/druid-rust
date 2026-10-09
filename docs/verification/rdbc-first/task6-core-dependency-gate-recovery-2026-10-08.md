# 核心默认依赖门禁恢复（2026-10-08）

继续现有五 crate 规格与 Task6 工具闭合，不创建规格、不关闭 P07，也不扩大默认依赖检查的验收范围。CodeGraph 当前源码显示 `verify_core_dependency_boundary.py` 仍调用不存在的 `druid-core` 包，与 ADR-CRATE-002 的当前 `druid` 核心不一致。

## 实际 RED 与修复

- 原真实脚本 exit1：Cargo 报 `druid-core` 不匹配任何 package。日志 `task6-core-boundary-cli-red-2026-10-08.log`。
- 新增合同自测首次4项中1 FAIL，明确暴露命令仍选旧 package。
- 将目标修正为 `druid`；使用脚本所在仓库作为 cwd，并保持 locked、normal edges、原13个禁止依赖及原遍历深度。没有删除 forbidden 项、加入豁免或改为只检查直接依赖。
- 进一步新增空输出／错误根包测试，实际出现两个 FAIL：脚本原先会错误输出 PASS。增加首个非空行必须为 `druid v...` 的根校验；Cargo 非零退出继续拒绝。

## 最终真实结果

`python -X utf8 scripts/test_verify_core_dependency_boundary.py` 最终5 PASS/0 FAIL，覆盖当前包、仓库定位与 locked 命令、逐个禁止依赖的间接违规、重复依赖、Cargo 错误，以及空输出／错误根包。前一4 PASS是中间状态，不累加为9个测试。

实际从仓库外运行修复后的脚本 exit0，输出 `PASS: druid default normal-dependency graph is clean.`。使用既有Rust1.97.1和offline环境，无工具安装。最终日志为 `task6-core-boundary-final-tests-green-2026-10-08.log` 与 `task6-core-boundary-final-cli-green-2026-10-08.log`；源码和日志SHA见 `task6-core-boundary-final-2026-10-08.summary.json`。旧RED及中间GREEN日志、summary均保留。

## 不扩大完成判断

该脚本仍检查默认 feature 的 normal dependencies；`config-http` 可选路径、all-features、完整DAG、公共API和发布门禁继续开放。Python工具自测不等于生产合同测试。

完整 C2 runtime5 原session55218/Python41348持续运行；先观察到Wrapper deadpool目标，02:35:53又观察到Cargo47848执行 `task6_jdbc_agent_h2_contract_test_parity`。两项Python工具文件未修改Rust/Cargo/proto/SQL或正式C2脚本、ledger、分母；1092项冻结输入再次逐文件一致。日志缓冲不构成停止证据，不重启原进程。

Task6仍FIX_REQUIRED，Task7八义务、Task8、P05/P06/P07共18顶层任务未关闭。未修改系统安全策略、安装工具、切换分支、stage/commit/push或发布。
