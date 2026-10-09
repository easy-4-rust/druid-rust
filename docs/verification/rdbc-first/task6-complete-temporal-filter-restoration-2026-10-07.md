# Task6 完整 decimal、temporal 与 Calendar Filter 重载恢复（2026-10-07）

本轮恢复固定原案例，取得实际行为RED及完整GREEN，正式绑定一项EXACT；Task6继续FIX_REQUIRED，完整18顶层任务保持OPEN。

## 固定合同及实现

原始BASE为5afe8609b177544ff7d18461aa84f69efe8ee90e，案例pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads，原函数SHA-256为b542f91c16f6a9cca5df42dd32ab3c2fd02e6c2cb1e2af95b37415944a76c038。

完整保留19个等值断言、四种decimal重载、十二种date/time/timestamp重载、scale7/8、Calendar未指定与指定null/UTC、全部16条有序原调用记录、精确filtered temporal failure错误及实际创建Statement异常计数1。真实SQLite原查询仍为SELECT 1 AS value，不先定位行，Filter必须真正短路。旧接口参数映射到唯一标准ResultSetExt与ResultSetRead，没有删除原观测。

Druid ResultSetFilter新增默认委托standard_result_set_read hook，公开原标签和完整读取描述符并接受同步后续调用；不公开raw所有权，不新增驱动接口树。标准读取链先按注册顺序进入完整参数hook，默认委托再执行既有value around hooks和真实驱动读取。未配置列治理的标签在完整hook之后才定位；已配置隐藏列/映射时仍先验证治理。资源读取不进入标量转换。

该公开接口增补按现有规格§13发布前修正规则追踪，规格已记录实际合同；公共API差分、外部消费方总体矩阵、迁移表和SemVer allowlist门禁仍OPEN。新增外部实现者的真实运行测试不能替代全体API验收。

## 实际运行证据

日志均在.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

| 日志 | 终态 | 范围 |
| --- | --- | --- |
| task6-complete-temporal-filter-red-2026-10-07.log | session89453 exit101，0PASS/1FAIL | 原完整案例第一读取实际失败：游标未定位行，短路Filter未被调用 |
| task6-complete-temporal-filter-green-2026-10-07.log | session91963 exit0，143PASS/0FAIL | Druid lib106、FilterStat21、完整Recycle16；由最终边界回归替代 |
| task6-complete-temporal-filter-final-2026-10-07.log | session12007 exit0，145PASS/0FAIL | lib107、FilterStat21、Recycle16、外部Filter实现1 |
| task6-complete-temporal-filter-matrix-retirement-2026-10-07.log | session71913 exit0，1PASS/0FAIL | 已绑定原完整案例的旧通用占位退役后矩阵仍实际运行 |
| task6-complete-temporal-filter-wrapper-2026-10-07.log | session6730 exit0，10PASS/0FAIL | Wrapper完整lib回归 |

最终三个范围互不重叠，共156PASS/0FAIL/0ignored。新增私有控制证明隐藏列不能被短路绕过、失败只记录一次、关闭后不再进入hook；新增外部crate实现控制证明无需私有链类型即可实现hook，真实SQLite未定位行时按未知标签短路、默认委托保留value hook顺序、错误按注册链回退。

最初候选选择器误用短函数名，验证器拒绝未登记的attributed target；修正为实际完整库测试路径后候选源码观测PASS。未放宽检查规则、未移除断言。

## 正式账本和剩余

全部1283项前七基线字段固定。该原完整案例绑定至crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs#core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads，旧通用矩阵中的对应占位ID已移除。正式1123EXACT/160PARAMETERIZED，反向方法索引及派生Markdown同步。新鲜完整源码观测审计1260PASS/23FAIL，JSON为task6-complete-temporal-filter-source-floor-audit-2026-10-07.json。

迁移权威79、Task7pending25、Physical标准化73及原SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3不变；标准纯度48文件PASS；迁移验证器17自测与权威5自测PASS，diff检查PASS。

新鲜C2 --no-execute session7241终态exit1，首败推进至pooled_result_set_preserves_distinct_n_string_filter_overloads。该命令不执行运行测试，不作为全行为验收。

上述测试使用stable Rust1.99.0、locked/offline，不替代MSRV1.95、默认1.97.1、all-features和完整workspace。公共API工具链、C1/C2、SemVer、登记委托顺序/跟踪证据、最终审查、Task7八类义务及Task8/P05–P07继续OPEN。下一步恢复完整NString Filter重载，随后继续其他22项，不用通用矩阵占位替代原合同。
