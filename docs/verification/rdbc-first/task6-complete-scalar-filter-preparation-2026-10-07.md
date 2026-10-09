# Task6 完整标量 Filter 合同准备（2026-10-07）

状态为 PREPARATION / NOT_COMPILED / NOT_RUN / NOT_BOUND。没有修改运行中完整workspace的Rust/Cargo输入，也没有减少21项源码合同缺口或关闭18个顶层任务。

## 完整原合同

固定BASE为 `5afe8609b177544ff7d18461aa84f69efe8ee90e`，原案例为 `crates/druid/tests/result_set_filter_semantics_test.rs#pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads`。

完整候选保留19个assert_eq、2个assert、String/Boolean/Byte/Short/Int/Long/Float/Double/Bytes九类各下标和标签共18条有序事件。成功返回值仍分别为filtered-index/filtered-label、true/false、11/12、21/22、31/32、41/42、51.5/52.5、61.5/62.5、[7,8]/[9,10]；失败仍为精确DriverError(filtered double failure)，真实创建Statement异常计数1。

候选使用未定位的真实Toasty SQLite `SELECT 1 AS value`，显式标准Statement注册/admission/lease/policy，保留原连接id73和sqlite-scalar-filter上下文。仅把原驱动下标转换为ColumnIndex及标准调用形态，保持原所有观察；cleanup经真实ResultSet、Statement、Connection关闭。Filter使用已存在standard_result_set_read，不新增第二驱动接口，未知读取描述符默认委托。

补充治理候选遍历九类描述符的下标/标签18入口：隐藏列拒绝且不进Filter、每次实际失败记错一次（18），后续精确短路错误总19；关闭后不追加日志或计数。另用仍存活结果集令lease失效，18入口先拒绝，不进Filter或原生读取，计数0。这些数字是待运行验收，不是通过证据。

## 已验证与接续

原完整函数保存为 `complete_scalar_filter_base_original.rs.txt`。三个Rust候选和原提取文件SHA、原BASE函数体SHA见 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/complete_scalar_filter_preparation_manifest_20261007.json`。

原19/2断言计数与18事件完整保留检查通过；三个Rust候选经rustfmt语法处理，格式后哈希更新。语法格式检查不能代替编译、执行或原合同等价验收。

同一完整workspace session6505及cargo69032持续运行，编译已推进到Admin/Bench测试。990项冻结的crates/vendor Rust/Cargo与根Cargo/toolchain输入fresh全部未变；docs/scripts不在该快照范围。等待该执行句柄终态，不重启或把超时当失败。

终态后先记录完整结果，再接入完整标量案例及治理测试。实际RED若暴露治理计数或路由问题，再修复相同标准路径；如果原完整合同已通过，只能认定新增行为证据，不伪造生产RED。只有完整回归和BASE源码观察核验后才绑定EXACT。后续动态Statement、metadata、资源读写、navigation和row mutation等剩余Filter合同仍必须完成。

## 补充准备：默认原生委托与注册顺序

新增独立 `ScalarReadProbe` 候选，九类下标/标签原生入口分别记录原请求和目标；通用object读取明确失败，防止默认委托退回通用转换后仍被相同返回值掩盖。标签find_column记录与返回23保留，原生标签入口须继续收到virtual而非被下标重载替代。

新增两个FilterAdapter默认委托控制，九类18入口返回独立探针的原值并核对27条原生日志。普通标量标签的既有value-hook列定位保留，不能套用NString无额外定位规则。Unicode原生字符串包含emoji，两次成功读取的Java UTF-16计数应为14，作为待运行的真实上下文观察。

新增OrderedScalarFilter A/B注册顺序控制，九类分别验证下标和标签的正向进入、原生调用与反向退出；native double标签fail须保留精确scalar probe failure，B/A反向error，实际Statement计数1。另使用未定位真实SQLite游标，A/B/C中B返回完整SQL NULL，九类18入口必须仅有A/B四条事件，不进入C/find_column/原生游标，计数0。

新增三个Rust草稿经rustfmt语法处理，六个Rust候选及原完整提取文件共七个artifact格式后哈希已更新；仍NOT_COMPILED/NOT_RUN/NOT_BOUND。990项运行中的workspace冻结输入fresh未变。session6505与cargo69032继续存活，最新实际rustc27056确认正在编译task6_mock_driver_parity，先前Admin/Bench编译并不意味着完整构建已结束。没有开启第二次Cargo，也没有改变正式1125/158、1262/21或18个开放顶层任务。

## 完整候选源码观察核验与后续缺口

再次从固定BASE解析完整原函数，并用正式验证器的观测函数核对候选：断言分布精确相同（19个assert_eq、2个assert），18条原事件列表顺序逐项相同，完整调用/断言密度下限通过。产物为 `drafts/next_filter_contract_preparation_audit_20261007.json`。这是草稿源码观察，不代表Cargo目标注册、编译或行为通过；未执行verify_ledger绑定，不减少21项正式缺口。

完整动态Statement Filter及metadata平台句柄原函数也按BASE保存，包含函数体SHA、断言/调用字段及原提取artifact SHA。动态Statement原合同包含默认创建语句身份、拥有型替换对象的身份/叶子接口、精确filtered statement failure及创建语句计数1。CodeGraph当前源码 `DruidPooledResultSet#get_statement` 仅直接返回逻辑owner，不进入Filter，`ResultSetFilter`不存在statement around-hook，因此后续需要实际路由恢复，不能以现有owner身份测试或账本占位代替。

metadata原合同包含独立原生meta_data调用、Filter短路返回零列句柄、精确filtered metadata failure及计数1，以及真实SQLite双列默认委托；这些完整观察后续均须实际验收。已有standard metadata around-chain只是实现基础，不证明完整原案例通过。

完整workspace6505仍LIVE：cargo69032与先后rustc53344 driver_extension_registry_test、64664 rdbc_result_set_stat_test、50504 task6_pooled_callable_coverage_test_parity父子关系及句柄确认持续推进。990冻结输入fresh未变，未重启或改变运行源码。

## 完整候选接入准备

`drafts/apply_complete_scalar_contract_20261007.py` 已保存，仅经Python语法检查，尚未执行。其前置条件要求当前Object完整workspace真实summary存在且TERMINAL_PASSED/exit0/failed0、日志哈希吻合、990冻结输入及全部草稿manifest哈希吻合；若已有同名测试或夹具则拒绝重复覆盖。满足后一次接入原完整标量、全入口治理、默认/顺序/错误三项测试及三个各自独立的真实夹具对象文件。它不修改生产路由，也不更新账本；实际RED/GREEN与完整原观察核验仍须随后完成。

这不是生产实现或合同完成证明。本轮同一session6505仍运行，cargo69032及连续编译目标generic_execute_row_count_test、task6_pooled_statement_semantics_test_parity、sqlx_deadpool测试已核实。原完整回归源未变，正式缺口/任务分母不变。
