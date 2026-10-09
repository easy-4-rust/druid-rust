# ClientInfoStatus canonical 归属修复

RDBC 标准层补齐 java.sql.ClientInfoStatus 的四种真实失败状态。Druid 的历史 sql::client_info_status 模块及 sql::ClientInfoStatus 门面重导出同一类型，避免两套定义；未改变 SqlClientInfoException 的既有字符串 payload。

新增跨公开路径的直接类型赋值、HashMap 属性失败状态和 TypeId 测试。RED 原33983 exit101/E0432，GREEN 原16500 exit0/2PASS；完整 RDBC all-features 原25608 exit0/474PASS0FAIL（18结果块，含external implementor、public surface与doctest）。workspace fmt check实际exit0且无输出。

严格workspace/all-targets/all-features Clippy原96459最终实际exit0/3m22s，-Dwarnings通过。新冻结1094源码+58控制输入终态逐项无变化。原LIVE机器摘要保留为历史观察；[最新终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-client-info-status-recovery2.summary.json)取代它。

使用新Cargo check明确返回的rmeta重编译原probe_04.rs，实际rustc exit0；[原探针复验](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-client-info-status-probe1.summary.json)。

[原命令、SHA与机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-client-info-status-recovery1.summary.json)。

历史37/31/1 canonical诊断对应此前1092输入；本修复证明ClientInfoStatus的公开路径与同一标准身份，不宣称其余目标在新输入重新通过。canonical账本和oracle未修改。定义归属改变需要最终API/SemVer门禁核验，历史路径保持仅为源码兼容证据。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务继续OPEN。
