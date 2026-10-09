# Task6 完整 NString Filter 重载恢复（2026-10-07）

本轮恢复原完整NString案例并正式绑定一项EXACT，取得两种真实行为RED和最终相关回归633PASS/0FAIL。Task6继续FIX_REQUIRED，完整18顶层任务仍OPEN；新的完整workspace回归已启动，终态待确认。

## 原合同与实现

固定BASE为5afe8609b177544ff7d18461aa84f69efe8ee90e，原案例pooled_result_set_preserves_distinct_n_string_filter_overloads，函数SHA-256为ca9bc33bd2ca7db3004d0cd79a8c488443a14cda8b4fd91afce152663c8601c0。

完整保留原9个等值断言、真实Unicode游标推进、默认索引与标签不同返回值及两条有序驱动调用、两个短路重载及日志、精确filtered NString failure错误、实际creatingStatement.exception_count1以及原真实SQLite查询SELECT '你好，Druid'。短路查询仍为SELECT '原值'，不先定位行，原列3和标签name保持不变。

ResultSetRead新增独立NString请求，ResultSetExt的索引与标签便利方法分派到唯一标准read_object/read_object_by_label。默认分派继续经过object并只接受SQL NULL、NString与Scalar(String)，不使用getString的缓冲标量转换、不把整数字符串化。Druid将该请求纳入完整读取hook，链末端保留Unicode，不把资源句柄扁平化为值。

原驱动探针find_column会记录physical:find_column:{label}并返回23。本轮最初新探针遗漏这项记录；首次单例GREEN和632项回归因此不能关闭完整原标签合同。补齐原记录后真实RED证明多出find_column:name。最终NString在自己的完整hook之后直接委托原生getter，不额外进入需要定位列的value hooks。普通String及其他已有描述符的value hook顺序仍由独立真实外部测试验收，没有为了通过而删除原两条驱动日志断言。

新私有探针和Filter各为独立文件，默认Rust模块定位；没有恢复Physical标准树或公开raw所有权。该公开穷举枚举的新变体按规格§13显式发布前API修正追踪，API/全外部消费方/compiler identity/SemVer allowlist门禁继续OPEN。

## 实际运行证据

日志均位于.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

| 日志 | 终态 | 证据范围 |
| --- | --- | --- |
| task6-complete-n-string-filter-red-2026-10-07.log | session3322 exit101 | 新探针缺必需next，编译失败；不作行为RED |
| task6-complete-n-string-filter-runtime-red-2026-10-07.log | session99494 exit101，0PASS/1FAIL | 原第一读取返回通用对象入口而非索引-NString |
| task6-complete-n-string-filter-green-2026-10-07.log | session5319 exit0，1PASS/0FAIL | 初次单例GREEN，缺完整find_column观察，已被最终证据取代 |
| task6-complete-n-string-filter-final-2026-10-07.log | session57118 exit0，632PASS/0FAIL | 补齐原探针定位记录前的回归，非最终标签合同验收 |
| task6-complete-n-string-filter-label-red-2026-10-07.log | session27277 exit101，0PASS/1FAIL | 原调用顺序中多出physical:find_column:name |
| task6-complete-n-string-filter-label-final-2026-10-07.log | session91486 exit0，632PASS/0FAIL | RDBC全包474、Druid lib109、FilterStat21、Recycle16、外部Filter2、Wrapper lib10 |
| task6-complete-n-string-filter-matrix-retirement-2026-10-07.log | session54323 exit0，1PASS/0FAIL | 正式独立绑定后移除旧通用矩阵的NString占位ID |

最后两个范围互不重叠，共633PASS/0FAIL/0ignored，其中包含RDBC既有生成测试435及1个doctest。该数量不作为C1所有生产方法验收证明。

新增默认控制验证Unicode/NULL/was_null、整数严格拒绝、未定位与关闭错误、缺失标签错误；私有池化控制验证隐藏列不进入hook、真实创建语句失败计数、关闭后索引及标签均不再调用hook。外部crate控制使用真实Toasty SQLite和公共池装配，证明NString与String描述符独立、虚拟标签短路、错误按注册链回退、默认Unicode/NULL不额外进入value hooks，以及普通String仍保留既有value顺序。

## 正式账本与源码审计

原案例正式绑定至crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs#core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_preserves_distinct_n_string_filter_overloads。只登记这一实际库测试选择器，没有放宽源码观察规则。

全部1283项前七基线字段验证不变；正式1124EXACT/159PARAMETERIZED，反向方法索引和派生迁移Markdown同步。新的完整源码审计1261PASS/22FAIL，JSON为task6-complete-n-string-filter-source-floor-audit-2026-10-07.json，未覆盖任何历史审计。剩余22项仍全部为ResultSet Filter。

完整C2 --no-execute session42246终态exit1，首败推进至pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity；该检查不执行Cargo测试，不能当作全行为验收。

标准纯度48文件PASS；源码闭合672文件及无重复标准树子检查PASS。完整no-Physical公共API/rustdoc门禁继续OPEN，子检查不替代整体验收。权威表79、Task7pending25、Physical标准化73及固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3保持PASS；迁移验证器17自测和权威5自测PASS，13个本轮Rust文件格式检查PASS。

## 全仓与剩余任务

完整workspace默认locked/offline串行no-fail-fast回归已启动：session34164，日志task6-complete-n-string-filter-workspace-2026-10-07.log；当前仍确认LIVE，未取得终态。运行期间冻结Rust/Cargo输入。此前3759PASS/1FAIL属于旧源码，不用作本轮全仓验收。

最终按仓库原配置执行git diff --check，exit0；16个本轮跟踪输入（13个Rust源文件及3个根Cargo/toolchain文件）哈希复核不变，清单为drafts/n_string_filter_workspace_inputs_2026-10-07.json。该清单只覆盖声明范围，不冒充全workspace输入快照。迁移验证器17自测和权威5自测重新执行均exit0；实际rustc版本1.99.0 (b940084d7 2026-09-28)。完整workspace同一session34164仍确认LIVE，不重启、不将观察等待超时当作终态。

本轮使用stable Rust1.99，不替代MSRV1.95、默认1.97.1或all-features验证。Task6的22个源码缺口、C1/C2、API/compiler/SemVer、登记顺序/Wrapper跟踪证据、最终审查，以及Task7八类义务、Task8、P05–P07继续OPEN。下一原完整案例是Object Filter重载；先确认同一全仓命令终态，再修改Rust输入。
