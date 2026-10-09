# Task 6：强类型读取与资源更新重载恢复（2026-10-05）

沿用 ADR-RDBC-006 和 single-standard-unification 规格/计划。固定 BASE 为 5afe8609b177544ff7d18461aa84f69efe8ee90e；完整原案例 all_strong_getter_overloads_preserve_raw_spi_argument_identity 来自 druid_pooled_result_set_semantics_test.rs。

## 最终实现与原合同

完整保留58次驱动调用的顺序、参数和原断言：Decimal 无scale/scale7/8，Date/Time/Timestamp 的 Unspecified、Specified(None)、Specified(Some(Asia/Shanghai))，八类可空资源读取、七类可空资源更新，以及 BlobStream/ClobReader/NClobReader 的 unspecified/long 长度101–106。探针只记录实际标准入口调用，没有预填调用或伪造计数；实际DruidPooledResultSet通过真实ToastySQLite Statement/registry/admission接管它。

唯一标准 ResultSet 增加 ResultSetRead typed descriptor 与 read_object/read_object_by_label。ResultSetExt 保留原列名、scale、Calendar状态，资源NULL更新保留具体family，三种LOB stream/Reader不再折叠到普通流family。Druid转发保持开放/租约检查、隐藏列和正反列映射、原生列名入口、资源锁、policy错误分类。标量读取复用既有value around-hooks及原顺序/返回值替换，资源不扁平化为标量。

最终默认转换核对的是固定BASE的 crates/druid-rdbc/src/spi/rdbc_result_set.rs：Decimal with_scale；日期时间保留字符串、Date、Timestamp替代转换；Calendar描述符抵达驱动，默认读取已解码naive值时不再次解释时区。最初引入的scale/Calendar默认拒绝不符合BASE，已撤回；对应失败证据保留，不将较小的拒绝行为作为完成标准。原生适配器可覆盖typed入口解释Calendar。

动态Box<dyn ResultSet>补充测试验证两层Filter的a_before/b_before/b_after/a_after顺序、Decimal返回值替换、负scale原样分派、Calendar原样分派、资源原列名读取、隐藏/缺映射无驱动调用、配对映射和关闭后拒绝。真实BufferedRowSet补充三项测试验证默认scale/Calendar/替代时间转换及错误、七类资源NULL更新、流拒绝不消费/不关闭调用者资源。

## 已执行验证

日志目录：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

- task6-strong-getter-initial.log：初始26个编译错误，缺标准typed合同/资源variant/scale方法及一个未适配调用；不是运行时RED。
- task6-strong-getter-runtime-red.log：session84120终态exit101，0PASS/1FAIL，强类型入口被折叠到通用result_set_value。
- task6-strong-getter-green.log：session2243终态exit101，补充夹具误用不存在的set_driver_column_map，三处编译错误；已按实际既有setter及方向修正。
- task6-strong-getter-green-final.log：session65590终态exit0，完整原案例和动态Filter/治理补充2/2PASS。
- task6-strong-getter-default-red.log 与 task6-strong-getter-temporal-default-red.log：两个独立运行均exit101/0PASS/1FAIL，暴露不符合BASE的默认拒绝行为；恢复原默认转换。
- task6-strong-getter-rdbc-final.log：session46404终态exit0，16结果块，471PASS/0FAIL/0ignored，含1doctest及新默认合同3项。包括既有435项生成方法测试，不据此宣称C1完整生产证据。
- task6-strong-getter-regression-final.log：session38068终态exit0，6结果块，126PASS/0FAIL/0ignored：Druid lib79、FilterStat21、metadata ownership3、buffered matrix6、Filter coverage16、Filter canonical matrix1。
- task6-strong-getter-c2-final.log：session44986终态exit1，下一处 custom_typed_object_checks_the_physical_column_exactly_once 缺明确分支。完整C2仍FAIL。
- 验证脚本自测15/15PASS；final purity检查44文件PASS；authority79行、Task7pending25行、Physical disposition73行，固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc不变；15文件rustfmt --check及git diff --check通过。

新的全Workspace locked/offline/no-fail-fast/jobs1/serial-tests回归已启动，确认LIVE session56913，task6-strong-getter-workspace.log；尚无终态结果。旧metadata版3677PASS/2FAIL早于本次及此前string/constants/update修改，不能作为当前源码全量证明。

## 剩余范围

原1283分母、摘要和关键观察保留；本案例改为明确私有lib准确入口，删除旧矩阵phantom case ID，白名单仅增加准确函数；反向方法索引和派生文档同步。目录现1082exact/201parameterized，这不是通过率。结构性最低缺明确分支62：buffered24、lease2、recycle12、ResultSetFilter24；已存在分支仍需完整行为验证。

Task6仍FIX_REQUIRED；Callable原prepared5vs6、完整C1/C2、公共API/compiler/SemVer、all-features/MSRV、完整审查未关闭。新的公开descriptor/core方法/update variants需要最终API门禁。完整目标仍为Task6–8和P05–P07（18个顶层任务），未缩减。下一项是完整custom typed object默认读取一次及原UnsupportedOperation合同；不能用通用拒绝或不读取替代。无提交、推送、发布、安装或切换分支。

Task6 strong-getter fullworkspace terminal 2026-10-05: session56913 exit101 /241 result blocks /3689 PASS /2 FAIL /0 ignored incl2doctests. Only failed target task6_callable_canonical_matrix_test; original prepared5vs6 retained. Supersedes LIVE56913. Default suite is not fully green; C1/C2/API/compiler/features/MSRV/review and full Task6-8/P05-P07 remain open.
