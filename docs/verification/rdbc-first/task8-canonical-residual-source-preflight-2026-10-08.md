# canonical 剩余缺口源码预检（2026-10-08）

这是现有 P04 Task8 的诊断证据，不创建新规格、不改变69对象分母或验收标准。完整 workspace 回归正在原句柄15014运行，预检期间不修改冻结源码。

## 已确认的实现与缺口

| 对象组 | CodeGraph 当前源码事实 | 下一步所需证明 |
| --- | --- | --- |
| DriverAction | 唯一真实 trait 已移入RDBC driver；产品路径重导出；实际注销合同1/0、完整RDBC477/0 | 用新Cargo artifact重跑原外部探针；当前41/27/1全量诊断属于迁移前，不能直接改成42/26/1 |
| JDBCType | RDBC types/rdbc_type.rs 定义真实RdbcType enum，包含Java8类型、vendor/name/number逻辑；inventory目标是JdbcType | 核实完整类型和数值映射；真实定义的canonical命名与旧路径身份、所有使用方和API兼容性一起验收 |
| RowId | RDBC types/rdbc_row_id.rs 定义真实RdbcRowId；按bytes实现Eq/Hash，get_bytes返回副本；inventory目标是RowId | canonical身份、复制隔离、相等/hash及实际参数/结果资源接线；不能新建独立重复DTO |
| PseudoColumnUsage | 四状态enum真实定义仍在Druid sql层 | 唯一标准归属、旧路径身份与四状态语义 |
| SQLData/SQLInput/SQLOutput/Struct | Druid SqlData trait引用具体SqlInput/SqlOutput并返回DruidError；输入输出当前为具体缓冲struct；Struct保存有序Value属性 | 按Java oracle落实标准trait与实现分工、声明顺序、null/转换错误、资源/UDT映射；不能把现有struct别名标成TRAIT通过 |
| ConnectionEventListener/StatementEventListener | Druid core trait当前接收ID及DruidError；后者保留Java Druid仅存储/移除/reset清空的历史发布行为 | RDBC不得反向依赖Druid；标准事件对象源身份/错误payload与产品监听策略分别建模并验证 |
| StatementEvent | Druid core/filter.rs现有同名enum描述Create/Prepare/Execute/Close等Filter动作 | 此对象不是javax.sql.StatementEvent标准DTO，不能借同名直接重导出来填补inventory |
| Types | RDBC已有Types公开对象，但types子模块为私有，inventory将其列为MODULE公开路径 | Java常量公开合同、选定标准路径和可见性需真实认证；不凭import解析推定全部常量等价 |

资源六对象的TRAIT/别名冲突、RowSet系列及其他失败目标继续保留在原编译诊断中；此表不缩小剩余范围。

## 权威证据范围

- CodeGraph查询：rdbc_type.rs、rdbc_row_id.rs、pseudo_column_usage.rs、sql_data.rs、sql_input.rs、sql_output.rs、struct_value.rs、connection_event_listener.rs、statement_event_listener.rs及filter.rs的当前磁盘源码。
- [完整原探针诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes2.summary.json)：68个Rust目标41PASS/27FAIL，另1项非Rust目标未认证。
- [DriverAction归属恢复](task8-driver-action-ownership-recovery-2026-10-08.md)：其原失败公开路径已有真实RED/GREEN，但尚未重新运行完整探针。

下一步先取得当前完整回归终态与输入一致性，再执行fresh artifact探针。名称、类型形态、来源身份、真实行为、API/SemVer分别需要对应证据。final CLI仍未接入；Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。

## 冻结 Java 来源复核

从oracle固定OpenJDK commit取得JDBCType、Types、RowId、PseudoColumnUsage原文件，四个source SHA均精确匹配oracle；39个enum名称/Types编号与验收草稿逐项一致。getVendor原逻辑返回java.sql。Java JDBCType还包含valueOf(int)查找及未知编号IllegalArgumentException；已补入草稿39项查找和4个未知值拒绝，尚未运行。

RowId源语义要求考虑来源表/数据源和有效期；toString格式由驱动决定。已有Rust get_bytes副本策略的测试不构成Java规定必须复制或同byte跨数据源必定同一行的证明；真实驱动字符串表示/生命周期继续未认证。

[来源机器核验](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-java-source1.summary.json)、[更新后的未执行草稿](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-contract-draft2.summary.json)。

## 全量 Java 来源准备终态

已取得固定OpenJDK commit的全部69个原始Java文件，共1,320,455字节。每项source SHA、fqcn和normalized declaration均与经过load_oracle校验的冻结记录一致，java.sql52/javax.sql17，无下载或身份错误。原文件保存在F构建证据目录task8-java-jdbc42-source，未改写包或原始源码。此结果只证明来源输入齐备，不认证全部方法、Rust对应逻辑或真实数据库。

[69文件来源机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-source-acquisition1.summary.json)。原始F文件路径、URL和SHA均包含在证据中。1099源码+58控制输入仍变化0。


## 三个标准值对象的实际编译 RED

使用原完整 workspace Cargo 子进程实际 --extern 指向的 RDBC/Druid rlib，直接 rustc --test 编译已有合同草稿，实际 compiler exit1/E0432：JdbcType、RowId、PseudoColumnUsage 三个 canonical import 均缺失。草稿 source SHA 未变；1099 源码及58控制输入前后变化0。此结果是真实外部编译 RED，测试函数尚未进入运行，也不是 Cargo integration target 的 RED/GREEN 或最终验收。

下一步待原 workspace 回归终态后，将相同完整合同接入 Cargo 测试，再迁移唯一真实定义；保留39种 JDBC 类型 name/vendor/number/value_of、未知编号拒绝、旧路径类型身份、RowId 当前 bytes/hash/copy 策略及四种 PseudoColumnUsage 状态。RowId 驱动来源/有效期/字符串表示仍需独立认证，不以 bytes 合同替代全部 Java 语义。

[直接编译 RED 机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-contract-direct-red1.summary.json)、[原始诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-value-contract-direct-red1.log)。当前全量 canonical 诊断为42PASS/26FAIL/1非Rust目标未认证；早先41/27/1描述为历史。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。


## 固定 Java 类型来源的实际运行基线

使用机器已有 Oracle JDK 8（java/javac 1.8.0_321），编译 oracle commit 7373384725418fa645fb4508ec05a869bc7fc3e7 的五个原始源码：JDBCType、Types、SQLType、PseudoColumnUsage、RowId。未改包或源码，使用 -Xbootclasspath/p 将实际编译类加载到 bootstrap；辅助程序返回每个运行类的 class 字节SHA，与本次 javac 输出逐项一致，避免误用宿主JDK内置类而声称固定来源已运行。

实际 javac exit0、java exit0。39种 JDBCType 的 name/vendor/number 及 Types 常量、39项 valueOf 查找、4个未知编号的 IllegalArgumentException、4种 PseudoColumnUsage 状态全部通过；实际39个输出与Rust合同草稿名称/编号逐项相等。1099源码+58控制输入前后变化0。

此证据仅完成Java类型值基线；Rust运行、完整Java/Rust differential、RowId驱动来源/有效期/字符串合同、其他JDBC方法及真实数据库均未因此认证。Oracle JDK 8运行时版本明确不同于固定OpenJDK源码tag，固定的五个类型通过实际bootstrap class SHA核验。

[Java运行机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-standard-value-runtime1.summary.json)、[原始输出](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-standard-value-runtime1.log)、[辅助源码](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-standard-value-runtime1-helper.java)、[编译日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-standard-value-runtime1-compile.log)。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。


## 当前真实 Rust 类型值与固定 Java 运行输出差分

使用原workspace Cargo编译子进程的精确RDBC rlib，编译并执行独立Rust程序，逐个调用已有RdbcType的SqlType方法；程序没有填入预期数值，实际输出与上一节固定来源Java运行的39个TYPE记录比较。rustc exit0、程序 exit0；39类型的name/vendor/vendor_type_number逐项一致，differences为空。原源码及58控制冻结前后变化0。

此结果证明现有RdbcType三项值行为与固定Java运行基线一致，后续canonical迁移应保留它们。仍缺JdbcType标准公开路径、Rust value_of成功/拒绝行为；三个标准对象编译RED保持有效。该独立差分不替代正式druid-bench Java/Rust JDBC42 differential目标、全方法覆盖、数据库认证或最终Task8验收。

[实际差分机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-type-value-differential1.summary.json)、[Rust原始运行输出](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-type-value-differential1-runtime.log)、[Rust编译日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-type-value-differential1-compile.log)、[Rust辅助源码](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-type-value-differential1-helper.rs)。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。


## 数值查找行为缺口的独立编译RED

使用同一真实RDBC产物，直接调用已有RdbcType::value_of(4)及value_of(9999)，rustc实际exit1/E0599，证明现有真实enum也缺少数值查找方法；不是仅由canonical JdbcType名称缺失导致。原Java运行已分别证明39项查找及4项unknown拒绝，Rust完整合同草稿保留相同要求。1,099源码+58控制输入前后变化0。尚未修改生产实现或进入行为运行，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。

[独立lookup RED](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-existing-type-lookup-red1.summary.json)、[实际编译诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-existing-type-lookup-red1.log)。


## 69对象完整编译与签名基线

已有Oracle javac8对固定OpenJDK来源69个原始Java文件实际编译exit0，69个主类均生成。内部sun.reflect API及旧弃用相关编译警告保留原始日志；不声称零警告或固定tag的整个JDK已构建。

用Java8 bootstrap prepend加载本次产物，Class.forName(name,false,null)读取69对象，运行时每个类资源SHA与本次编译主类逐项一致，helper javac/java均exit0。反射提取公开、非synthetic的声明方法1060（其中default27）、公开声明字段189、公开构造148；统计不含继承方法的重复计数，输出保存实际generic签名和default标记。

此基线用于后续69对象形态/资源/事件/RowSet及真实差分合同；没有调用这1060个方法或148个构造，不认证方法行为、Rust对应关系、数据库或完整Task8。1099源码+58控制输入复核变化0。原方法迁移479/原测试1283分母不受此Java反射统计影响。

[全量Java编译](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-compile1.summary.json)、[保留编译诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-compile1.log)、[实际签名与类身份机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-signatures1.summary.json)、[原始反射输出](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-signatures1.log)、[反射辅助源码](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-jdbc42-signatures1-helper.java)。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。


## 27个Java default方法的实际代表性输入运行

对上述固定来源69类签名中全部27个default方法，用JDK8 MethodHandle直接执行实际default body。7个owner接口运行class SHA与固定来源编译产物相等；辅助编译及运行均exit0。代表性参数完整记录（int=1、long=0、String=probe、SQLType=JDBCType.INTEGER、Class=Object.class、Object=null、数组为空）。代理会记录并拒绝任何抽象driver调用；本轮abstract_calls均0。

实际结果：19个SQLFeatureNotSupportedException（原SQLState为null）、5个UnsupportedOperationException、2个数值0返回、1个false返回。27个运行签名与已提取default签名集合完全相等，原始逐项结果保留。必须分别建模Java default unsupported与driver SQL feature unsupported；不得将此结果改写为全部0A000或统一一种异常。此运行只覆盖已记录代表性参数，不证明所有输入或数据库driver覆写行为；尚未完成Rust对应差分。

[27项实际默认行为机器结果](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-default-method-runtime1.summary.json)、[原始输出](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-default-method-runtime1.log)、[调用辅助源码](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-java-default-method-runtime1-helper.java)。1099源码+58控制输入变化0，Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务OPEN。
