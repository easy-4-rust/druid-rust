# Task6 完整 Object Filter 原合同准备（2026-10-07）

接续状态：下文是准备阶段历史记录。完整workspace34164已终态失败3774/8，相关范围复测34/0和15/0。Object草稿随后接入实际Cargo，接口RED及行为111/3RED后路由114/0；完整原案例正式EXACT绑定，绑定后回归115/0，账本1125EXACT158PARAM，完整sourcefloor1262PASS21FAIL。C2已推进完整scalar getter Filter，整体仍失败。详细证据见 `task6-complete-object-filter-restoration-2026-10-07.md`，不能把下文历史NOT_RUN/LIVE状态当当前事实；原准备manifest及冻结输入保留。新的完整workspace6505仍运行，尚无终态。

当前阶段为准备，未接入 Cargo、未编译、未运行、未绑定正式迁移账本。完整18个顶层任务保持开放；正式源码观测仍为1261PASS/22FAIL、1124EXACT/159PARAMETERIZED。

## 原合同与候选

固定BASE为5afe8609b177544ff7d18461aa84f69efe8ee90e。原完整案例为crates/druid/tests/result_set_filter_semantics_test.rs#pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity。

完整原函数、ObjectShortCircuitFilter及类型映射身份辅助逻辑已独立保存。原文件SHA、提取函数SHA和五个草稿SHA见.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/complete_object_filter_preparation_manifest_2026-10-07.json。函数块SHA仅标识本次提取范围，不替代正式账本的原案例哈希。

候选保留原全部10个等值断言：七个成功重载返回值、七条有序成功事件、精确DriverError("filtered object failure")和真实创建语句异常计数1。方法命名只映射为标准object_with_type_map_by_label、object_as、object_as_by_label；预期事件仍为原Java重载名称，不随Rust方法名称更改。

候选装配实际Toasty SQLite连接、已登记标准Statement和逻辑策略，保留连接id75及sqlite-object-filter上下文名；查询仍为未定位的SELECT 1 AS value。清理调用真实ResultSet、Statement和Connection关闭入口。

补充第11个等值断言观察空类型映射、非空类型映射、自定义目标类型和String目标类型的实际借用地址。候选Filter记录原参数引用，不通过重新构造内容模拟参数身份。

读取描述符候选区分Ordinary、TypeMap(None)、TypeMap(Some(empty))和Typed原引用。该候选属于Druid Filter扩展，不增加第二套物理驱动标准；返回完整RdbcObject，不能把资源或不透明对象扁平化为Value。公共hook及描述符尚未进入生产，后续须按既有规格§13追踪API及外部消费方门禁。

## 已验证及未验证

- 原10个断言和原有序事件的草稿保留检查通过。
- 三个标准候选草稿经rustfmt语法处理通过；这不是类型检查、编译或运行证明。
- 五个草稿文件哈希已在格式处理后更新。
- 原回归声明范围的16个输入哈希全部不变；该范围不冒充整个workspace快照。
- 准备脚本首次冻结输入检查因仓库根目录计算错误失败，修正后实际检查通过；它不是产品行为RED。

同一完整workspace session34164通过工具handle和实际cargo PID18164确认仍运行，日志为task6-complete-n-string-filter-workspace-2026-10-07.log，尚无终态。不重启该命令，不修改运行中的Rust/Cargo输入，不将等待超时视为终态。

## 接续要求

同一workspace取得终态后，先记录完整结果及所有失败。随后接入完整Object测试，取得实际失败证据，再实现完整对象Filter around-chain；补齐默认委托、短路、注册顺序/错误回退、资源及不透明对象返回身份、隐藏列与关闭/过期租约控制。只有实际合同回归通过并核对原源码观察后才绑定EXACT，不提前减少22个缺口或关闭Task6。

## 补充准备：六入口治理及资源默认委托

新增guard草稿使用真实SQLite结果集，分别观察六个对象入口在隐藏列配置下的精确拒绝、Filter和参数记录均为空、实际Statement异常计数6。清除治理后原typed标签失败应记录一次实际异常，合计7；关闭后六个虚拟读取均应拒绝且不增加记录或异常。这些数字是待运行的候选验收值，不是已有运行结果。

新增默认委托探针及测试草稿，使用实际已登记Statement包围原生ResultSet探针，经过两个默认Filter验证全部六个入口。各入口返回相同CharacterStream或Custom对象，原映射/目标引用保留，原生日志中不能出现额外find_column。Reader候选进一步读取前两个UTF-16单元后从原句柄读取剩余内容，验证共享游标而非复制值；Custom候选比较底层对象实际地址并downcast读取原payload，防止用字符串替代任意对象。

七个候选Rust文件fresh rustfmt --check通过；九个草稿文件SHA已更新，原完整10个断言和补充参数身份第11个断言保留。主案例候选的imports及调用记录读取已补齐，避免引用只存在于其他测试局部的calls辅助函数。所有候选仍NOT_COMPILED/NOT_RUN/NOT_BOUND，不作为行为GREEN。完整原回归的16项声明输入哈希fresh全部相同。

同一session34164仍LIVE，实际cargo18164和依次推进的rustc子进程已核实，当前最近观察子进程70380。完整回归仍在编译，无终态结果；不改变正式1261/22、1124/159或完整18项开放状态。
