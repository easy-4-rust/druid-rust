# JDBC公开入口可重复编译门禁（2026-10-08）

新增scripts/verify_jdbc42_public_entries.py与对应CLI合同测试，将此前一次性探针转为仓库内可重复执行工具。本工具是公开入口诊断，不是最终语义/完整形态/运行行为认证，也不补写LIVE总账。

## 实际TDD与当前结果

- 初次9项CLI合同实际9FAIL：入口脚本缺失，Python返回2，原日志保留。
- 实现后同9项实际9PASS；采用真实rustc编译测试rlib和各条目，不以模拟编译器返回码作为通过依据。
- 相关test_*jdbc42*.py完整发现运行44PASS/0FAIL。
- 当前69行总账与实际RDBC rlib运行，门禁实际exit1：47PASS、21FAIL、0ERROR、1NOT_RUST_TARGET，与此前原公开探针诊断一致；未把失败改为skip。

[RED/GREEN及相关回归收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-public-entry-verifier-recovery1.summary.json)

[当前逐项结果及原编译命令](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-public-entry-verifier-current1/summary.json)

## 合同边界

TRAIT/CALLBACK必须通过实际Rust trait bound，具体对象或别名不能冒充；DTO只证明公开类型可解析，MODULE/ENUM_KIND只证明公开import可解析，明确不认证完整形态、variant或字段。唯一host排除目标单独记录，不计Rust通过。Rust目标采用严格路径语法，拒绝未知shape、空总账及重复身份；已有输出目录拒绝覆盖；全部失败收集后返回非零，编译超时单列ERROR，输入文件终态摘要变化拒绝PASS。

收据记录semantic/artifact/rustc SHA、实际工具版本、每项源码/log SHA及命令。调用者指定的artifact来源尚未由本脚本绑定Cargo构建与生产源码，因此artifact_source_provenance_certified始终false，不能单独作为Task8 final证据。总账来源绑定、真实API形态、行为、数据库及SemVer仍由其他正式门禁承担。

## 复现

```powershell
& 'C:/Espressif/tools/python/python.exe' -X utf8 scripts/verify_jdbc42_public_entries.py `
  --rustc 'C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustc.exe' `
  --artifact 'F:/codex_build_cache/druid_rust/task6_rust_1971_20261007/debug/libdruid_rdbc.rlib' `
  --dependency-dir 'F:/codex_build_cache/druid_rust/task6_rust_1971_20261007/debug/deps' `
  --output-dir '<新的不存在的证据目录>'
```

本次新增两个Python控制文件不属于原1104源码+58控制冻结，原冻结逐项变化0；未扩大原Clippy/Cargo回归对新增工具的认证。原workspace54741尚无终态，Task6 FIX_REQUIRED、Task7八义务、Task8 INCOMPLETE和18顶层任务全部保留。原日志、逐项探针与机器摘要均按原字节镜像，rmeta仅保留F盘。

## 输入解析与摘要绑定竞态修复

新增实际输入变化测试：真实解析器返回后替换临时总账，再使用真实rustc编译已解析内容。修复前10测试9PASS/1FAIL，错误收据为PASS_PUBLIC_ENTRIES_ONLY；解析旧条目但记录新文件SHA，终态无法发现漂移。修复后摘要直接绑定实际解析的原字节，测试确认即使编译通过也必须FAIL并列出变化输入，原字节摘要保留。未修改Rust夹具来凑结果。

完整合同10/0，相关JDBC42回归45/0。current2重新执行当前69行，实际exit1/47PASS21FAIL0ERROR1宿主排除；逐项item_key/status/probe_source SHA与current1一致。原证据保留，新的137个探针、原日志和摘要逐字节镜像。新增两Python文件摘要更新，原1104源码+58控制冻结变化0；不扩大原workspace验收范围。

[竞态RED/GREEN与回归收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-public-entry-input-race-recovery1.summary.json)

[修复后当前完整入口诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-public-entry-verifier-current2/summary.json)
