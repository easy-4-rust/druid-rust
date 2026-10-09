# Task7 其余标准资源候选与来源核查（2026-10-08）

本次只准备F盘候选及外部编译合同，生产标准资源仍未迁移，原取消释放失败未修复，Task7八义务及18顶层任务全部保留。

## 固定Java声明与接口边界

五个固定OpenJDK源码逐字节SHA匹配原69对象来源收据。实际声明数量为Array11、Clob13、NClob0（继承Clob）、Ref4、SQLXML9。加上此前Blob11共48个声明，不含NClob继承的方法重复计数。

- Ref不定义free，候选不凭空添加JDBC释放方法；Druid内部owner治理与兼容句柄扩展应单独处置。
- NClob继承全部Clob操作，不增加新的方法。
- Clob使用现有真实RdbcString保存UTF-16单元，包括孤立代理项；不能退化为UTF-8 String再用字符数量替代Java单元数量。
- SQLXML#getSource/setResult允许class参数null，返回厂商特定的默认Source/Result；候选使用Option保留该请求，不能悄悄指定Stream。

[五来源声明、SHA及候选映射](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-canonical-resource-candidates1.summary.json)

## 当前生产SQLXML空class请求实际RED

当前公开RdbcSqlXml::source(None)与result(None)使用实际RDBC产物编译，分别E0308：要求非空RdbcXmlRepresentationType引用。两个错误实际exit1，尚未进入运行。固定Java源码getSource第364–365行及setResult第406–407行明确null含义，因此这是合法请求表达能力的缺口，不能以Rust禁止null作为完成证明。

[原编译收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-sqlxml-null-representation-red1.summary.json)

## 候选严格编译

F盘分别保存Array/Clob/NClob/Ref/SqlXml五个trait候选，每个对象一个文件；JDBC方法均为必需接口声明，不放置空实现或todo。可变操作统一&mut self，实例要求Send+'static，不要求Sync。Rust能力扩展明确标注；XML Class参数使用Option。

独立metadata探针实际rustc -Dwarnings exit0且零输出：五个trait均能作为Box<dyn Trait>，NClob可提升为Clob可变视图，SQLXML source(None)/result(None)返回的future均满足Send。此为类型/对象安全/参数合同编译，未证明任何实际Array/Clob/Ref/SQLXML方法行为、状态机或数据库支持。UTF-16实际运行、SQLXML单次读写状态及流完成绑定仍须正式测试。

[候选实际编译收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-canonical-resource-candidates2.summary.json)

## 后续接入与冻结

全仓原54741仍运行，1104源码+58控制输入逐项变化0。候选不进入生产Cargo模块，不修改semantic inventory，不提升LIVE状态。

原全仓终态后需先将外部合同接入正式测试，再协调迁移标准资源、兼容句柄真实内核、Connection与结果值返回链、现有access实现/66测试候选以及Druid owner包装。旧access必须最终收敛，不能保留为第二套驱动端口；单纯alias或候选编译通过不能关闭Task7。

## Clob/NClob 外部合同及宿主差分补充

当前生产公开 Clob/NClob 合同探针实际编译 exit1，5处 E0404，尚未进入测试运行。F盘候选以真实 Cell 状态构造 Send、非 Sync 的资源，严格 rustc --test -D warnings 编译 exit0、零诊断，四项测试实际4PASS/0FAIL：NClob继承与可变视图、UTF-16孤立代理项和偏移、1-based模式搜索、全部12个非free方法的关闭错误优先级及重复free一次释放。此为候选夹具合同，不是生产接入或数据库认证。

[生产RED与候选合同原收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-standard-clob-candidate-contract1.summary.json)

独立Java宿主对照首次 javac exit0/java exit1，SerialClob.setString(1, emoji, 1, 1)抛出ArrayIndexOutOfBoundsException。失败日志保留；第二次保持相同输入，仅捕获并记录异常以继续观察后续状态。固定来源Clob/NClob接口运行class SHA与原编译收据一致，但具体SerialClob来自宿主Oracle JDK1.8.0_321，不能冒充固定OpenJDK构建。

Java与Rust候选各8条实际记录，3条差异（写入结果、写后内容、truncate后内容），收据状态FAIL_DIFFERENTIAL。Java/Rust辅助程序各exit0只表示记录完成，不能解释为差分通过。

[首次宿主运行失败](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-java-clob-utf16-runtime1.summary.json)

[相同输入差分失败](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-clob-utf16-runtime-differential1.summary.json)

现有JDK javap -c -l -p实际exit0，原字节码保存且镜像。setString四参数方法先在offset4调用substring(offset)，再转char数组（偏移4–17）；写入时又以原offset+i索引该数组（偏移151–158），对应源码line432。对两UTF-16单元emoji、offset1、len1，截取后数组仅1单元，却访问下标1，与原异常一致。这是宿主实现原因的字节码证据，不是固定来源SerialClob语义结论，也不授权修改候选去模仿宿主越界。完整Java/Rust和真实驱动差分继续开放。

[宿主字节码原始日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-host-serial-clob-bytecode1.log)

[捕获命令与SHA](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-host-serial-clob-bytecode1.summary.json)

## SQLXML一次性读写合同实际RED

固定OpenJDK SQLXML.java来源SHA再次核对为f261795c70afbf99fc46f050e92618f6662d3038882c769be6890e1e1d998ae0。第167–179行要求读写至多一次：任一读取API消耗readable，任一写入API消耗writable；同时禁用另一方向由实现选择，测试不强制选择。第193–201行要求free使对象无效，重复free无操作。

使用当前Cargo来源RDBC rlib，独立rustc --test -Dwarnings实际编译exit0、零输出；运行exit101，1PASS/3FAIL/0ignored。测试直接调用生产ResourceHandleFactory::sql_xml、RdbcSqlXml::clone、string/get_string/set_string/free。观测access保留真实XML值和委托计数，不自行实施一次性门禁。

| 合同 | 实际结果 |
| --- | --- |
| 同句柄第二次string读取拒绝 | FAIL，第二次成功，底层读取2次 |
| clone别名读取已消耗值拒绝 | FAIL，第二次成功，底层读取2次 |
| clone别名第二次set_string拒绝 | FAIL，第二次成功，底层写入2次 |
| free后读写拒绝且重复free只释放一次 | PASS，底层读写均0、release1 |

这证明当前公共共享句柄不统一实施一次性状态，不能据此断言所有真实驱动均违规；驱动access可能自行维护状态。正式标准资源迁移须明确状态所有权，保证同一实例/克隆不能绕过合同，不能仅增加全局capability位。此测试尚未接入Cargo，不认证流关闭与execute绑定、全部Source/Result表示、数据库或Task7完成。

[原编译、运行与来源收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-sqlxml-one-shot-native-red1.summary.json)

[真实失败日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-sqlxml-one-shot-native-red1.log)

[可执行合同草稿](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-sqlxml-one-shot-native-red1-helper.rs)


## 现有 registry 与 ResourceContext 的取消对照（2026-10-08）

CodeGraph 确认现有 `StandardResourceRegistry` 已被 Statement/ResultSet 及 holder close cascade 使用；其 coordinator 持有 queued actor，等待调用者取消不等于取消清理。不能只给未接入生产的私有 context 再添加一个独立 supervisor。

本次独立 rustc harness 直接 path 包含 11 个未修改的 Druid 私有生产文件，使用同一个 ReleaseActor、ResourceContext 和 ControlledOwner 比较两个入口。严格 `-D warnings` 编译 exit0（仅显式允许独立包含模块的 dead_code），零编译输出。四项实际运行 exit101：

| 路径 | 驱动释放窗口取消 | owner 通知窗口取消 |
| --- | --- | --- |
| 直接 ResourceContext release | FAIL：后继250ms观察超时 | FAIL：后继250ms观察超时 |
| registry register_batch + close_registered_after_admission | PASS | PASS |

两项 registry PASS 同时断言 driver calls/completed 各1、owner calls/completed 各1、Freed、重复close无重放；清理中有open resource且不能开启下一代，最终close_all排空后才能开启generation1。未无限时观察直接路径，不宣称实验已经证明无限等待。

同一二进制另运行直接包含的原生产单测，13 PASS/0 FAIL，未过滤其中任何一项；四个新增对照测试在该独立命令中显式skip，不能把13/0替代原四项2/2结果。

范围：真实私有生产源与受控资源actor的源码集成对照，不是公开资源/完整Druid Cargo集成，不是所有驱动认证。新actor仅存在于证据helper中，尚未迁移canonical资源/返回链路，也未实现生产free接入，原公开Blob取消RED、Task7八义务及Task6/Task8状态不变。后续应将真实资源payload交给现有registry，并验证显式free与父级close竞态、owner错误/重试、过期generation及Send非Sync驱动桥接。

证据：[机器收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-context-cancellation-differential1.summary.json)、[实际对照日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-context-cancellation-differential1.runtime.log)、[原单测日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-context-cancellation-differential1.existing-tests.log)。Helper、编译/运行日志及收据逐字节镜像；原1104源码+58控制输入复核变化0。全仓原54741继续运行，未重启、取消或修改冻结输入。


## Send 非 Sync Blob 与父级关闭并发证据（2026-10-08）

在上一节11个未修改生产模块基础上，harness以 `tokio::sync::Mutex<Box<dyn candidate Blob>>` 独占驱动payload，外层实现现有 `StandardResourceHandle`，通过实际 registry 的 register_batch/close_registered_after_admission/close_all 执行清理。候选Blob是F证据文件，不是当前RDBC已发布或生产canonical实现；不得扩大本次范围。

夹具Blob保留 `Cell<usize>`、实现全部十一项Java Blob方法，先真实读取length=4，再进入释放测试。严格编译exit0/零输出，实际两项并发测试2PASS/0FAIL（原13单测在这条命令中被名称filter排除，上一节原单测13/0证据独立保留）：

- 显式释放进入驱动窗口时，父级close_all同时开始；显式调用者取消后，父级排空成功。
- 显式释放进入owner窗口时，父级close_all同时开始；显式调用者取消后，父级排空成功。

两项均验证父级在gate释放前尚未完成且不能复用generation；gate释放后底层free实际调用1次、owner进入/完成各1次、context Freed、底层读拒绝、重复逻辑/父级关闭无重放、最终可开启generation1。对父级JoinHandle的20ms等待超时不取消父级任务；实际完成等待上限500ms仅为本地测试观察预算，不是生产SLA。

同源negative cfg编译实际exit1/E0277，明确 `Cell<usize>` 不满足Sync；正向actor有Send+Sync+'static编译断言。这证明内部同步actor可拥有Send非Sync资源，不能把registry的Sync要求直接施加于canonical Blob接口。未通过unsafe、改变驱动trait bound或删去Cell使夹具变Sync。

证据：[机器收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-send-only-blob-parent-race1.summary.json)、[实际运行日志](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-send-only-blob-parent-race1.runtime.log)、[非Sync负向编译](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-send-only-blob-parent-race1.negative-sync.log)。五个helper/log/summary逐字节镜像，原生产11文件SHA及1104源码+58控制输入变化0。

仍缺canonical资源生产接入、公共返回链路、正式Cargo测试、全部资源/驱动、owner错误重试与过期generation等认证；Task7八义务和18顶层任务仍开放，不能将候选actor误标为完成。全仓原54741继续运行，不重启或取消。


## owner终态错误保留候选：实际RED/GREEN（2026-10-08）

本次先以11个未修改生产私有模块和受控actor执行3项：驱动释放失败保留资源并重试、owner错误返回活跃等待者且不重放、取消等待者后晚到父级的错误观察。前两项验证实际行为；第三项是诊断：当前context在owner返回错误后标Freed，registry再关闭该actor获得Ok，晚到父级看不到原错误。诊断3/0不能记为错误处置安全验收。

在相同源码上补候选actor验收：owner终态错误须继续对父级可见，并禁止静默进入可复用下一代。真实RED编译exit0、运行exit101，1PASS/2FAIL；失败正是普通owner错误与取消等待者owner错误丢失。原driver失败重试控制PASS。

候选修复只在F/helper actor内：增加异步close_gate串行发布结果；仅当context已Freed而返回Err时保留owner终态错误，后继close继续返回原错误；驱动失败时context仍Open、不锁死重试。成功时才注销。没有修改实际ResourceContext/registry，不将驱动已成功释放回退为Open，不重新调用owner去掩盖错误。

相同三项测试body SHA保持不变，GREEN1严格编译exit0/运行3PASS0FAIL。GREEN2增加上一节的registry驱动/owner取消控制，严格编译exit0/零输出、实际5PASS0FAIL。该正向命令显式排除两个旧直接context取消RED和13原单测；旧RED2/2失败及原单测13/0仍独立保留，不把filter当全部通过。

证据：[初始错误诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-release-error-policy1.summary.json)、[真实RED](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-owner-error-retention-red1.summary.json)、[候选GREEN1](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-owner-error-retention-green1.summary.json)、[候选GREEN2五项](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-registry-owner-error-retention-green2.summary.json)。四组helper/compile/runtime/summary共16文件逐字节镜像；11生产源及1104源码+58控制输入变化0。

范围限制：这是拟接入Druid resource actor的错误保留策略候选，不是已经接入的公共生产资源，不认证实际owner的connection discard/错误分类或全部driver。终态失败的处置必须接入真实父级owner并验证，不能只让registry永久保留条目。标准资源返回链路、非Sync payload完整组合、正式Cargo及实库验收仍缺；Task7八义务/Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。原全仓54741仍保持运行。


## 完整RDBC源码候选：六资源真实trait与外部消费者（2026-10-08）

为保护仍在运行的全仓1104/58冻结，独立复制当前RDBC src全部120文件到F/task7-rdbc-source-candidate1；没有创建Git worktree/分支，也没有修改原工作区Rust/Cargo/正式测试。实际直接rustc使用Cargo JSON返回的七依赖产物编译完整未修改副本exit0/零输出，再对六个标准名称施加trait bound，真实RED exit1/六处E0404。

在该完整源码副本中，将Array/Blob/Clob/NClob/Ref/SqlXml别名替换为此前按固定Java声明准备的真实可变Send+'static traits；NClob继承Clob，Ref不额外增加Java不存在的free，SQLXML保留可空representation参数。六旧Rdbc句柄原逻辑移入单独rdbc_*.rs，仅去除原标准名称alias；mod.rs仅声明/重导出。候选共126源文件，改7原文件+新增6文件。完整候选库严格-Dwarnings/forbid unsafe编译exit0零输出，相同六trait探针exit0。

Blob和Clob/NClob外部消费者测试只改import，移除path包含的独立trait定义，直接链接本次完整候选库公开接口；原测试body不变，两目标各4PASS0FAIL，共8/0，严格编译零输出。涵盖Send非Sync/可变Send future/dyn/NClob上转、Blob范围与模式资源、UTF16与未配对surrogate、free幂等/关闭拒绝、unsupported流与capability。不能扩大为Array/Ref/SQLXML全部运行语义或真实驱动认证。

完整69行公开入口工具在候选产物上实际exit1：53PASS/15FAIL/0ERROR/1非Rust排除；仅六资源trait入口从原47/21/1诊断改善。当前原工作区仍为47/21/1，不能将候选结果登记为生产LIVE或Task8 final通过。工具对DTO/enum/module的有限范围与artifact来源认证限制保持原声明。

可重建交付物：[候选完整源码包](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-candidate1.zip)、[综合机器收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-candidate1-validation.summary.json)、[候选源码编译收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-resources1.summary.json)、[候选全入口收据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-public-entries1/summary.json)。13个改/新增源码另保存drafts/task7-rdbc-source-candidate1/src/resource便于review；包只含126源文件、manifest和trait探针，不含rlib/exe。各基线/编译/外部测试原始证据及137入口probe/log/summary逐字节镜像。

尚未完成：旧Access与句柄仍是待迁移输入，尚未实现canonical资源返回链路、Druid logical owner actor、SQLXML统一状态或全资源驱动合同；不能把暂存两个层次的候选当成最终唯一标准树。候选还未取得Cargo整仓/正式测试/API/SemVer验收，不应用到生产冻结。120原副本来源与1104源码+58控制输入逐项变化0，原全仓54741继续运行；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


## Connection 三类 LOB 创建返回链路候选（2026-10-08）

本轮继续完整 RDBC 源码候选，修改其 `connection/connection.rs`：`create_blob/create_clob/create_n_clob` 从旧具体句柄改为 `Box<dyn Blob/Clob/NClob>`，使用 canonical resource trait imports，补充中文 Java 来源和 owned 返回说明。默认不支持行为保持真实 `SQLFeatureNotSupportedException`。候选完整126源严格 `rustc -D warnings --forbid unsafe_code` 编译 exit0、零输出；产物保存新 `candidate2` 目录，不覆盖此前 candidate1 产物与源码 zip。

### 实际 RED/GREEN

相同外部验收源码先针对六资源 trait 已存在、Connection 仍返回旧句柄的 candidate1 编译：实际 exit1，三处 E0053 分别指向 Blob/Clob/NClob factory。另有五个因旧返回句柄方法使用 `&self` 产生的 downstream unused_mut 诊断，原日志保留；初次收集器误把解释 footer 的 E0053 算作第四项而 assertion exit1，摘要现按 `error[E0053]` 精确记录三项，不将收集器误差描述为额外生产失败。

相同验收源码针对完整 candidate2 编译 exit0、零输出，实际运行 **11 PASS / 0 FAIL / 0 ignored**。包含此前完整 Blob/Clob/NClob 八项合同，以及三个新增合同：

- `Connection` 与返回资源均容纳带 `Cell` 的 Send 非 Sync 实现，三个创建 future 满足 Send；释放 caller 后返回资源仍可使用，证明不借用 caller。
- 两次创建得到独立资源，释放/修改第一个不影响第二个；关闭夹具连接后三个 factory 明确拒绝。
- 仅使用 Connection 默认实现的夹具，三个 factory 都明确报标准不支持。

这些是外部接口、独占返回与完整夹具行为证据，不认证真实数据库、Druid owner、外部池父级关闭或所有 resource factory。固定 OpenJDK `Connection.java` 1100/1285/1302 行仍要求 SQLXML/Array/Struct 创建入口，尚待迁移；JDBC 没有 `createRef`，不得自行添加。旧 Access/句柄及 ResultSet/parameter 返回链路仍未退役。

### 可复现交付与边界

- [完整候选源码包2](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-candidate2.zip)：126源码、实际 Cargo manifest、外部验收及完整夹具；不含二进制。
- [机器验证摘要](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-connection-resource-return-contract1-validation.summary.json)：全部候选源码 SHA、bundle SHA、镜像 SHA 和原输入一致性。
- [外部验收源码](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-connection-resource-return-contract1.rs)。编译、运行与 RED 日志及命令收据按相同前缀保留。
- 可审阅补丁为 `drafts/task7-rdbc-source-candidate2/src/` 下14个真实文件（上一版本13个资源文件加 Connection）；生产未应用。

原120个 RDBC 来源文件 SHA 全部不变，原全仓冻结 **1104 源码 + 58 控制输入变化0**。原完整 workspace session54741 经同句柄实际等待仍 LIVE，不重启、不取消。生产入口诊断仍以原47/21/1为准，本轮未重新运行完整69入口或正式 Cargo/数据库验收。Task7八义务、Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务继续开放。


## SQLXML / Array 创建入口与完整 ExternalConnectionLease 候选（2026-10-08）

固定 OpenJDK Connection.java 1100/1285 行要求 `createSQLXML/createArrayOf`。本轮在完整 RDBC 候选中新增两入口，返回 `Box<dyn SqlXml/Array>`；默认真实返回标准不支持错误。Array 参数为 `Option<&str>` 与 `Option<Vec<RdbcObject>>`，保留 null 类型名、null/空元素数组、原顺序、typed 值与 SQL NULL，不在标准层猜测厂商转换。中文文档保留 Java 元素类型映射、转换/primitive 表示由驱动决定，以及新 SQLXML 初始无数据的语义。没有添加不存在的 createRef；Struct 标准形态/创建仍待完成。

### 实际验证

- 新外部合同针对 candidate2 实际 RED exit1，两个 E0407（无法实现缺失方法）和八个 E0599（无法调用）。针对完整 candidate3 严格编译 exit0、零输出，运行 **4/0**；覆盖 owned Send future、默认不支持、null/空区别、vendor 类型/typed 元素/SQL NULL 顺序及关闭前置拒绝。记录型夹具明确不支持实际资源创建，不冒充成功 SQLXML/Array 实现。
- 相同 candidate3 完整126源严格编译 exit0，原 LOB 返回合同/八个资源合同重新运行 **11/0**，原测试 body 不变。
- 完整当前 `ExternalConnectionLease` 源码逐字节复制到独立 F 文件，针对新 RDBC 先实际 RED：三 E0053 + 三 E0308。候选更换三个 LOB 返回类型并委托新增 XML/Array 入口后，完整该文件作为 library 严格编译 exit0、零输出。其 generated_keys cfg(test) 子模块不在此次 library 编译范围内。
- 独立外部消费者使用该完整 lease candidate library，严格编译 exit0、运行 **2/0**，另八项已执行的 fixture 测试按 `lease_contract_` 过滤（明确记录8 filtered）。证明三类 LOB 返回对象不借用租约调用方、五类请求到底层分派、Array null/empty/typed 参数保留以及底层 sentinel 错误原样返回、调用一次。使用 Box 所有权租约夹具，不代表实际 bb8/deadpool/数据库集成。
- 首次 lease 编译命令重复指定 edition，实际编译器参数错误 exit1，保存为 command-error1；修正命令后才取得上述真实 RED，不把工具调用错误计为生产诊断。

### 交付和未完成范围

[完整候选源码包3](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-rdbc-source-candidate3.zip) 包含完整126 RDBC 源码/实际 manifest、完整 lease 候选及五份验收/夹具文件；不含二进制。真实14个 RDBC 源补丁与1个 Druid lease 源补丁位于 `drafts/task7-rdbc-source-candidate3/crates/`，按生产路径组织。[机器验证摘要](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-connection-factories-lease3-validation.summary.json) 绑定固定 Java SHA、候选全部源 SHA、原 lease SHA、artifact 命令与24项逐字节镜像。此前两个源码包和产物均保留。

生产未应用，原120 RDBC 来源和全仓 **1104源码+58控制输入变化0**。原完整全仓54741再次经同句柄实际等待返回LIVE，无终态，不启动替代回归。生产47/21/1入口诊断不扩大到候选。仍需真实 SQLXML/Array/Struct 创建实现、Druid pooled factory/owner 与关闭错误分类、Access/句柄/操作返回链路退役、SQLXML状态、正式Cargo/外部池/数据库/API验收；本轮不关闭任何Task7义务或18顶层任务。


## 完整 Druid 源码候选与公开 Blob 的 registry 取消对照（2026-10-08）

本轮将验证从单文件 lease 推进到完整 Druid crate 源码。独立目录复制当前398个 source 文件、21个 resources 文件和真实 Cargo manifest；生产源逐项SHA一致。为避免不同 Cargo feature build 的 tokio/uuid 等重复产物混用，使用已记录真实 Druid all-features rustc 命令的31个依赖，分别重编译完整120源原 RDBC 与完整126源候选 RDBC，七个共享依赖保持一致。实际编译命令、环境和依赖/产物SHA均保留。

### 完整库类型检查矩阵

- 两个完整 RDBC crate 构建 rlib 均严格 exit0、零输出。
- 完整 Druid 原源码 + 原标准 RDBC：baseline3 严格 metadata 类型检查 exit0、零输出。
- 相同 Druid 原源码 + 新 RDBC：实际 RED exit1，精确三处 E0053 + 三处 E0308，均位于 ExternalConnectionLease 旧三个LOB返回签名/委托。
- 仅替换完整 lease 为此前已验收候选：完整 Druid candidate 严格 metadata 类型检查 exit0、零输出。

六个 cfg feature 来自当前 Druid manifest（default/sqlite/postgresql/mysql/turso/config-http）；此次为 library `--emit=metadata`，不含 cfg(test)、链接运行、完整 Cargo、wrapper 或真实数据库认证。首次准备遗漏 include_str resources 和第二次遗漏 Cargo 的 MSRV 环境变量分别保留为 baseline1/2 设置错误，不冒充生产错误；最终基线补齐真实资源及1.95 manifest值才获得上述PASS。

### 池化 factory 的实际缺口

源码绑定检查当前 DruidPooledConnection 的 Connection impl（1100行起），没有覆写 create_blob/create_clob/create_n_clob/create_sql_xml/create_array_of，继承标准默认不支持。类型检查通过不证明池化资源可用。不能只委托raw驱动对象：需要先接入独占标准资源代理、admission/registry 与 owner，确保父级关闭、取消、错误分类/丢弃和驱动/owner各自恰好一次。该缺口决定下一步实现方向。

### 公开标准 Blob 的真实监管接口对照

此前 parent-race helper 中单独定义的候选 Blob 引用替换为完整 RDBC candidate3 的公开 `druid_rdbc::resource::Blob`，不改变夹具操作和测试断言；实际包含的11个生产私有源不变。严格编译 exit0（保留 source-include 模式既有 dead_code 例外），两项 driver/owner 窗口取消与父级关闭测试 **2/0**，13个既有单测按精确task7过滤；负向要求 driver Sync 的相同源码实际 exit1/一处E0277。说明真实公开Send非Sync资源可以经异步Mutex交给现有registry监管，不要求第二套resource port。仍是候选 actor/夹具，不包含普通owner错误保留候选、真实pooled factory或正式Cargo集成。

### 交付与状态

- [完整 Druid + RDBC 候选源码包4](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-full-druid-rdbc-source-candidate4.zip)：完整398 Druid源、21资源、126 RDBC源和两个实际manifest，无二进制。
- [完整机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-full-druid-resource-candidate4-validation.summary.json)：原/候选来源SHA、编译矩阵、精确命令与环境、依赖/产物SHA、registry来源及16项镜像。
- [池化factory源码核查](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task7-pooled-resource-factory-source-audit1.summary.json)。

原398源/21资源及全仓 **1104源码+58控制输入变化0**。原全仓54741同句柄再次返回LIVE；候选编译/helper/打包本轮进程均已终态。未应用生产、未改oracle/inventory，不将类型检查当运行验收；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN保持。


## 标准 Blob 的受监管代理候选与既有资源关闭入口（2026-10-08）

本轮在 F 盘候选中新增 `DruidBlob` / `DruidBlobState`，分别承担标准 Blob 委托与受监管释放状态。每个对象独立文件；所有十一项 JDBC 方法均有真实委托，未创建空实现或默认成功 owner。驱动只要求 Send，内部异步 Mutex 提供 actor 同步边界。

Registry 新增 `close_registered` 候选：关闭已登记资源不再申请创建 admission，父级 Closing 期间仍可加入清理；与原 `close_registered_after_admission` 共用清理队列和等待逻辑。新入口先校验整批 registry 身份，拒绝不同连接相同 generation/id 的令牌，避免先清理部分本地资源后再失败。

验证证据：

- 原 registry 缺入口的验收编译 RED：实际 exit1/E0599；候选严格编译 exit0，18/0，包括原13单测及5项新增合同。格式化后原18项复验18/0。
- 删除候选身份检查的独立反例编译 exit0、运行 exit101/0PASS1FAIL；原测试体不变，验证了外来令牌防护的必要性。
- Blob 缺对象源码 RED 为文件缺失，不能冒充已执行行为 RED。首次夹具残留未使用导入的设置错误原日志保留；清理导入后严格编译 exit0/零输出，初始18/0，增加驱动失败重试和清理中读拒绝后20/0。
- Blob 公开标准方法验证：读写、区间写、搜索另一 Blob、截断、能力交集、流不支持、释放后拒绝、重复 free、父级关闭以及丢弃代理后父级清理。
- 取消窗口验证：公开 `Blob::free` 在驱动释放或 owner 通知时被取消，父级清理仍完成，driver/owner各一次。owner终态错误保留并由晚到父级观察，驱动失败则允许重试且不提前通知 owner。
- 结果发布使用 actor close gate，公开重复 free 等待终态结果，避免 Freed 可见但 owner 错误尚未发布的窗口。
- 相同 Cell 驱动负向 Sync 编译 exit1/E0277；候选没有增加外部驱动 Sync 要求。

可审查补丁：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/task7-druid-blob-proxy1/crates/druid/src/core/`。源码包 `task6-e-space-task7-druid-blob-proxy1-source.zip` SHA-256：`ecc0846862a0cf25517d5e8fb2f6168e9c69ae640650a6d62e383f31962d2e31`。机器摘要 `task6-e-space-task7-druid-blob-proxy1-validation.summary.json` 绑定源码、编译依赖、命令及21项逐字节证据镜像；exe/rlib仍仅保存F盘。

范围限制：这是独立 rustc/source-level 候选，不是生产池化工厂、Cargo目标、完整Druid新增模块编译或真实数据库认证。新代理还未挂入生产 `mod.rs` / Connection factory；没有关闭 Task7，也未认证 owner错误分类/连接丢弃。其他资源代理与 SQLXML状态仍开放。11个实际私有生产源SHA一致，1104源码+58控制冻结逐项变化0；原全仓回归54741本轮两次原句柄确认LIVE，未重启或取消。Task6 FIX_REQUIRED / Task8 INCOMPLETE / 18顶层任务OPEN。


## 完整 Druid 候选接入池化 Blob 工厂与真实 owner 策略（2026-10-08）

本轮将 `DruidBlob`、`DruidBlobState` 挂入完整候选模块树，新增独立 `DruidResourceOwner`。`DruidPooledConnection::create_blob` 先验证租约和创建 admission，再调用真实底层 Connection 工厂；驱动返回后同步建立身份、上下文和 owner 并交给 registry 接管。候选未修改生产工作树。

真实 owner 校验资源身份与类型、记录释放通知、将资源操作和释放错误交给同一 `StandardLogicalPolicy`；所有权异常或未释放即丢弃时停止租约并发送一次 discard。Context 的资源操作先检查 owner 租约；Blob 的真实读写错误与释放错误均分类，已失效租约拒绝继续读写，但仍允许受监管 free 完成清理。资源错误记录连接错误，不冒充 SQL 执行或 ResultSet 错误计数。

实际证据：

- 完整候选共401个Druid源码、21个资源文件、126个RDBC源码。两次完整 Druid rlib 严格编译均 exit0/零输出；第二次在格式化后的精确源码上完成并链接运行外部消费者。
- 初始三个 pooled consumer：原完整库编译0、运行101/1PASS2FAIL，失败为继承默认 `connection_create_blob` 不支持；候选扩展到6项后6/0。
- 最终同一验收源 SHA `8a8af1c5512f5228cd8071d3e07879e7b65131e129b1a4c2d2720635c37fbbc0`：原完整库编译0、运行101/1PASS6FAIL；新候选严格编译0、运行0/7PASS。没有用只编译接口替代实际委托与清理运行。
- 七项覆盖：两个资源相互独立、显式释放、父级关闭清理后归还、关闭后不触达驱动、公开free调用者取消而父级等待真实释放、创建/读取/释放fatal错误保留vendor code77并丢弃租约、真实holder归还再借出的旧资源隔离。fatal路径分别断言sorter和同步discard通知各一次。
- 将之前完整20项源级资源清理合同迁至新context/owner源码后的独立version2，严格编译0、20/0；保留两个异步取消窗口、错误保留和驱动失败重试。
- 首次隔离构建遗漏Cargo环境传递以及消费者首次缺少候选RDBC依赖搜索路径的设置错误原日志保留；它们不是生产行为RED。

候选相对zip4共11个Druid源变化，包含新增三个对象及已有registry补丁，ExternalConnectionLease随模块格式化产生字节变化。完整源码包 `task6-e-space-task7-full-druid-rdbc-source-candidate5.zip` SHA-256 `53e9bd5275fc210aff28bfb293c504883e5da6fafdfa78bf556c68972491446a`。独立补丁位于 `drafts/task7-pooled-blob-factory1/`；机器摘要 `task6-e-space-task7-pooled-blob-factory1-validation.summary.json` 绑定全部源码、31依赖、编译环境/命令、最终验收源、rlib和24项逐字节镜像。

限制：仅Blob工厂已在候选完整Druid中接入；Clob/NClob/SQLXML/Array等工厂及ResultSet资源返回仍开放。测试使用真实DruidPooledConnection/Holder与受控驱动、归还回调，不认证native pool worker、bb8/deadpool实池、正式Cargo目标或真实数据库；其他owner错误分类边界及SQLXML状态仍需完整矩阵。生产RDBC/公开入口仍为之前47/21/1，不能晋升为候选结果。1104生产源码+58控制冻结复核变化0；原全仓54741实际进程仍在，已从编译进入Admin测试，未取得终态。Task6 FIX_REQUIRED / Task7八项 / Task8 INCOMPLETE / 18顶层OPEN。


## 完整 Druid 候选接入 Clob/NClob 工厂（2026-10-08）

在隔离完整候选中新增 DruidClob、DruidClobState、DruidNClob 三个独立对象，覆写池化 Connection 的 create_clob/create_n_clob。Clob 十三项 JDBC 方法真实委托；NClob 继承 Clob 操作并保留 ResourceKind::NClob 身份，以 trait 上转保留原驱动对象。共用既有 registry coordinator、lease policy、真实 owner 分类与释放链路，不增加驱动 Sync 约束。

初始完整编译实际 exit1，两处 E0433 为误写 super::druid_blob 模块路径；原日志及收据保留。修复两处引用、rustfmt 后完整404个Druid源/21资源/126个RDBC源的严格 rlib 编译 exit0、零输出。这是候选实现错误修复，不记为生产行为 RED。

- consumer1 同源验收：旧 Blob 候选编译0、运行101/8PASS5FAIL；新候选编译0、运行0/13PASS。
- consumer2 保留上述十三项测试体，新增字符工厂 fatal 和 holder 再借用隔离；精确同源旧候选编译0、运行101/8PASS7FAIL，新候选编译0、运行0/15PASS。
- 新字符合同循环覆盖 Clob/NClob：UTF-16 长度/孤立代理项/输入 offset、资源 pattern 搜索、独立身份、free 幂等、父级等待清理、caller 取消、创建/读取/释放 fatal 分类各一次、驱动失败重试、关闭优先于不支持、关闭后工厂不调用底层、旧资源不跨 holder generation。
- 原七项 Blob 消费者回归同次保留并通过。Cell 字符驱动负向 Sync 编译实际 exit1/E0277；正向十五项无需 Sync。

相对 candidate5 精确五源变化：新增上述三对象及 pooled Connection/mod 声明。补丁在 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/task7-pooled-character-factory1/`。完整重建源码包 `task6-e-space-task7-full-druid-rdbc-source-candidate6.zip` SHA-256 `b9af3c2ab3a030fa486e1ab98a8fd51a7c827df5762e1e793dca892cc7202251`；机器摘要 `task6-e-space-task7-pooled-character-factory1-validation.summary.json` 绑定全部源码、31 extern、编译环境和命令、两个消费者源SHA、rlib及18项逐字节证据镜像。

范围：尚未生产 apply、正式 Cargo、native pool worker、真实 bb8/deadpool 或数据库认证；不宣称两异步 owner 通知窗口已对新增字符代理单独运行（真实 owner 通知无 await；此前 Blob 的 ControlledOwner 窗口证据保持原范围）。SQLXML/Array/Struct 创建、ResultSet 返回、SQLXML 一次性状态及旧 Access 迁移仍未完成。1104生产输入+58控制输入变化0。原全仓54741同句柄实际轮询仍LIVE，进程50420/41748在场；当前日志多项grpc FAIL但无终态，不重启/取消原进程。Task6 FIX_REQUIRED / Task7八项 / Task8 INCOMPLETE / 18顶层OPEN。

保存补充：E盘空间耗尽导致本报告写入失败并截断；已从原会话命令记录重建历史259行，前两个历史文档SHA检查点一致。新增内容及恢复前镜像均保存F盘。本轮zip6的E盘重复镜像已逐字节核对F盘原件后移除以恢复空间；源码包使用F盘原件，机器摘要另记录该镜像移除。

空间恢复后已从F盘原件重新建立zip6的E盘镜像并核对SHA一致；移除/恢复历史保留在机器摘要。


## 完整 Druid SQLXML 工厂与一次性状态候选（2026-10-08）

候选新增 DruidSqlXml、DruidSqlXmlState、DruidSqlXmlPayload 三个独立对象，并覆写池化 Connection::create_sql_xml。九项JDBC方法均真实委托；owner/lease/error/registry采用此前真实策略。读写机会保存于异步Mutex持有的payload旁：四种读API共享readable、四种写API共享writable；受支持调用在驱动await之前消费，caller取消与非致命错误不恢复机会。两方向独立，选择不强制交叉禁用；驱动自身仍可拒绝方向。关闭状态先于能力检查；明确不支持的stream不消耗可用字符串机会。

固定Java SQLXML.java SHA为f261795c70afbf99fc46f050e92618f6662d3038882c769be6890e1e1d998ae0，167–179行约束来源再次确认。HY010为本候选顺序错误选择，本次未认证其与全部Java驱动的SQLState完全一致。

实际验证：

- 完整407个Druid源、21资源、126个RDBC源候选，严格rlib编译exit0/零输出。
- 新consumer1旧完整候选RED：编译0、运行101/16PASS8FAIL；新候选编译0、24/0。旧十五项LOB测试体保留。
- 新consumer2同一验收源旧候选RED：编译0、运行101/16PASS12FAIL；新候选编译0、28/0，含原24项与新增写入取消、非致命错误、Source/Result原驱动身份、holder再借用隔离。
- 读取4×4和写入4×4组合均证明同方向只进入底层一次；read/write取消后同方向调用即时拒绝，不重放driver。非致命SQLState22000/code88错误保留且不discard，重复读取不再次调用sorter；相反方向仍可执行。
- UTF16孤立代理项、实际二进制/字符流读写和关闭、None/DOM/Stream/Custom表示、Source/Result原Arc身份、free后八种操作拒绝、父级等待、公开free取消、创建/读取/释放fatal分类一次、驱动free失败重试均有运行证据。
- 独立完整反例只删除payload的机会消费语句：库及相同consumer1编译0；XML九项运行101/6PASS3FAIL/15filtered，精确暴露重复读、重复写、取消后重读。未修改正式候选或断言。
- 同源Cell驱动负向要求Sync编译exit1/E0277，正向二十八项不要求驱动Sync。

初始生成脚本字符串转义语法错误在执行候选变更前修正，不计生产RED。新池化RED等待编译期间，按此前已实际运行的生产共享SQLXML一次性状态RED（1PASS3FAIL）推进实现；新池化RED最终也取得上述实际终态，并未提前声明通过。

补丁在 `drafts/task7-pooled-xml-factory1/`，相对candidate6精确五源变化。完整源码包 `task6-e-space-task7-full-druid-rdbc-source-candidate7.zip` SHA-256 `75b5585aeddc85beb89bf778bfeddd3bc38a9eb9121ce792438a77cdbaf9fe8d`。摘要 `task6-e-space-task7-pooled-xml-factory1-validation.summary.json` 绑定407/21/126源码、31extern、命令环境、两个验收源、反例唯一源差异、21项逐字节镜像及实际产物SHA。

范围限制：尚未生产apply/正式Cargo/实库/原生worker或真实外部池认证；本轮不修复旧RdbcSqlXml共享clone句柄或取消原Access迁移义务。未认证所有SQLXML表示转换、writer未关闭时Statement execute拒绝、驱动绑定期间生命周期、所有owner通知错误与跨资源返回链路。Array/Struct工厂、ResultSet资源返回、唯一资源trait树/旧Access退役和真实async结果集继续开放。

原54741完整workspace已终态Cargo101/3876PASS44FAIL/262块，1104+58冻结变化0；原日志及summary镜像已核对。失败为Admin grpc43项及pool lifetime idle_count1项；不把历史局部PASS覆盖此结果。源码冻结随原运行结束解除，但本轮仍未改生产，后续apply需新快照和回归。Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。


## 完整 Druid Array 工厂与子游标监管候选（2026-10-08）

新增 DruidArray/DruidArrayState 两真实对象，覆写池化 create_array_of，保留 nullable type_name/elements 与全部十一方法委托。能力为驱动与运行上下文的交集，owner/lease/error 分类沿用已验证真实策略。四种 getResultSet 重载返回标准池化 ResultSet，get_statement 为 None，保留索引/值/NULL/类型映射与范围参数。

raw 游标在 registry 异步登记之前同步移交 Array state，然后释放 Array payload 锁；失败清理不等待自己持有的锁。Array free 先等待操作移交，再直接关闭子游标 actor，最后驱动 free 和 owner 通知；不在同一 coordinator 中排队等待自身。父级关闭期间拒绝的新游标仍先归属并排空。

完整409个Druid源/21资源/126个RDBC源候选，显式Rust1.97.1与同源31extern，严格-Dwarnings/forbidunsafe rlib编译exit0零输出。先重建一致工具链基线并复验原XML/LOB消费者28/0，避免旧混合缓存E0514。相同最终consumer5旧完整库编译0/运行101，28PASS8FAIL；新完整库编译0/运行0，36PASS0FAIL。原28测试体保留，Array八项覆盖nullable输入、四值/四游标重载、父级清理、free取消、打开失败、fatal丢弃一次、显式free先关子游标、创建与父级关闭并发及无Filter元数据延迟。

consumer3的Debug格式要求导致E0277是夹具编译错误；consumer4的34/1失败来自无Filter时构造不会预读metadata的错误前提，不是产品缺陷。consumer5只为打开失败用例配置Some(FilterChain::new())并新增无Filter延迟报错对照，未改变候选生产行为或放宽断言。原编译与失败日志全部保留。

同源ArrayDriver含Cell，正向36项已实际链接运行；负向要求Sync实际exit1/E0277。独立完整反例只去掉Array state的子游标close_core调用，库及相同消费者编译0，精确显式free测试实际exit101/0PASS1FAIL，raw closes为0/期望1，证明不会由驱动free或父级close伪造该清理保证。未改正式候选。

精确五源差异补丁与验收源/helper/原日志/摘要在drafts/task7-pooled-array-factory1；完整源码zip8 SHA-256为59378b838a576ed1ba0c6ad5b538f53d142e3a488863a89bcba90b91f9e0deed，共556源码/资源输入，并逐个解压核验SHA。validation summary绑定源码、实际产物、命令、反例和逐字节镜像。

本轮仅独立候选，未生产apply、正式Cargo、native worker、真实外部池或数据库认证。Array能力稀疏矩阵、错误重试/旧租约再借用完整矩阵、Struct/Ref、ResultSet资源getter返回、旧Access退役和真实async流式读取仍开放，不将候选GREEN计为Task7完成。当前1167已枚举正式输入逐项变化0，未新增生产源。

原完整验证runner57728/C1子55648继续以实际进程观察为准；当前流水线尚无全仓终态。Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。


## Array 稀疏能力、错误重试与旧租约隔离（2026-10-08）

在完整Array候选源码不变的前提下扩展同源消费者：原36测试体逐字不变，新增六项，实际旧完整库编译0/运行101为29PASS13FAIL，新完整库编译0/运行0为42PASS0FAIL。

能力合同枚举READ/RANGE/TYPE_MAP/RESULT_SET全部16组合，对十个非free方法分别在Open和Freed状态调用，共320个操作断言；成功与不支持同时验证驱动调用计数，关闭错误先于不支持且不进入驱动。FREE始终支持，本轮不扩展到无FREE能力驱动或任意运行上下文掩码。

创建fatal保留08006/code77，Array创建工厂调用一次、sorter/discard各一次，后续创建不进入驱动；关闭连接创建零driver调用。读取fatal与非fatal分别保留08006/77和22000/88，前者失效租约拒绝后续底层读取，后者恢复后正常读取并可复用。释放两个错误分支均先关闭子游标，失败后驱动free可重试，子游标不重放关闭，最终free实际两次/子close一次，sorter一次，fatal才discard。真实返回holder再借用时，旧Array及游标拒绝使用/重复free-close不影响新代次，新资源与游标保持可用并各清理一次。

独立完整能力反例仅去掉DruidArray::require的不支持返回，驱动自身仍真实拒绝。409源库和同一consumer6严格编译0，精确稀疏矩阵实际exit101/0PASS1FAIL：不支持操作进入了驱动，调用数1/期望0。证明合同检查的是代理边界，不会被驱动最终返回相同0A000掩盖。候选409源码清单与原compile2逐项一致，反例唯一源码差异为druid_array.rs。

expanded6目录保存验收源、helper、真实命令/退出码/log及反例单源；证据zip SHA-256为b60485d479b2f5e12d4950bc8380b25e601a7e08f94174130fd09b39ae60598d，机器summary另绑定消费者/库/可执行产物SHA及原完整源码zip8。镜像逐字节核验，1167已枚举正式输入变化0。未生产apply、正式Cargo或真实DB认证；任务状态仍Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。下一步继续Struct/Ref与资源返回链路及正式接入，保持完整验收范围。


## Struct 标准对象及池化/外部租约创建链路候选（2026-10-08）

固定OpenJDK8u来源Struct.java/Connection.java的SHA逐项核对。Struct仅三个JDBC方法、不继承Wrapper、没有free，默认随应用引用有效；显式Map重载绝不使用连接关联Map。固定Druid1.2.28提交33824c3dec1612711f9bb4e409319bcab2e4cd0e的createStruct为checkState后直接委托，Git当前HEAD不同，因此另保存固定提交完整原件/方法SHA，不以当前检出替代基线。

新增独立resource/struct_value.rs标准Struct trait（Debug+Send+'static、exclusive &mut，不要求Sync），完整保留中文语义文档、属性声明顺序、SQL NULL和RdbcObject。Connection新增nullable类型名/属性数组owned工厂，默认明确不支持。Druid pooled工厂检查当前租约后委托并使用当前标准创建错误分类；返回对象无新增owner/registry/free状态机。ExternalConnectionLease原样委托，不嵌套原生池。已有druid::sql::Struct值快照没有冒充此trait，旧值类型退役/重导出仍待后续唯一树验收。

实际RED/GREEN：旧完整RDBC对同一外部消费者编译1/E0432与E0407，新增后完整127源严格-Dwarnings/forbidunsafe rlib0、同源四合同4/0。验证三方法、属性顺序/NULL/显式Map、nullable数组和类型名错误、typed driver错误、默认不支持、owned对象在连接关闭/销毁后仍可读取。含Cell的DriverStruct负向Sync实际1/E0277；擅加free负向实际1/E0407，原日志保留。

完整Druid基线与新候选都使用同一新RDBC重新严格编译0。原42测试体逐字不变，新增五项同源消费者：未接入Druid工厂的旧完整库43PASS4FAIL/exit101，接入后47PASS0FAIL/exit0；包含原全部Array/LOB/XML合同。新增池化类型名/顺序/映射、关闭后对象有效、NULL参数与原错误、关闭工厂零driver调用、创建fatal sort/discard一次及真实ExternalConnectionLease委托。此为当前标准创建错误策略的合同，不单独宣称Java所有异常分支/Filter回调差分已认证。

精确三RDBC/二Druid源差异在drafts/task7-pooled-struct-factory1，完整源码zip9含127RDBC/409Druid/21资源共557项，逐项解压核验SHA；zip SHA-256为3b06ca87385e5e0517977f5cc4a489d9a8ad11395ff0c26789f4370d5d48e470。Java原件、实际命令/环境/log/消费者/负向源逐字节镜像，validation summary绑定实际库/exe与源码SHA。1167已枚举正式输入变化0，未新增生产源。

本轮未生产apply、正式Cargo、真实数据库/native worker/外部实池或Java运行时差分认证。Struct嵌套资源返回、SQLData/类型映射完整矩阵、旧Struct值快照唯一归属、Ref代理/返回链路、旧Access退役及真实async流式读取仍开放。原完整C1/C2按原进程继续，Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。


## Ref唯一标准端口退役旧Access及Struct资源返回候选（2026-10-08）

固定OpenJDK8u Ref/Struct来源SHA逐项核对；Ref是SQL结构值的逻辑引用，取得句柄不能物化属性，全部四JDBC方法必须实现，没有Wrapper/free/Connection#createRef。固定Druid1.2.28提交33824c3dec1612711f9bb4e409319bcab2e4cd0e的ResultSet/Callable两组getRef重载均原样返回底层对象，仅getter本身错误交给checkException；完整Java原件与四方法SHA归档。

RdbcRef共享值句柄现在只持有Mutex<Box<dyn Ref>>，不再持有Access/ResourceLifecycle；删除rdbc_ref_access.rs、旧factory/context路径、Druid旧重导出及过时ResourceType::Ref分支。没有保留桥接driver接口。标准对象只要求Send，克隆共享身份及串行独占调用，Ref操作之外的FREE/STREAM位不对外宣称。不附加连接关闭失效或owner释放策略。新增RdbcStruct值句柄承载唯一标准Struct并加入RdbcObject::Struct，用于默认Ref目标及嵌套属性；保留属性延迟读取、SQL NULL、显式映射、克隆身份。PreparedInputParameter明确拒绝Struct的通用标量物化，原样保留对象供native adapter处理。

实际同源RED/GREEN：旧完整RDBC编译1/E0308旧Access入参及E0432/E0599缺Struct返回值；新完整127源最初E0004遗漏标量分支、随后-Dwarnings发现废弃ResourceType::Ref，修正后严格-Dwarnings/forbidunsafe完整rlib编译0，原七合同7/0。Cell驱动Sync负向E0277、擅加Ref free负向E0407、导入旧Access负向E0432均为实际编译失败。全部8种READ/WRITE/TYPE_MAP组合共32调用验证capability及驱动计数；仅删除句柄能力拒绝的完整127源反例仍严格编译0，同一精确矩阵实际0/1失败，唯一源差异rdbc_ref.rs。

完整409源Druid对新RDBC最初因旧重导出E0432失败；移除后严格完整rlib0。原47资源消费者源逐字保留，加入七Ref合同及两池化返回合同后实际56/0；进一步强化返回Ref的方法错误为08006/code96并新增参数/嵌套资源合同，实际58/0。池化ResultSet按下标/原列名返回同一标准Ref、父cursor/statement/connection关闭后仍由驱动语义决定，Ref目标Struct延迟属性、取消后共享锁释放、SQL NULL、原错误、Ref/setObject参数身份和嵌套Struct属性身份均有实际证据。08006原错误不额外触发Druid discard且连接保持可用，对应Java直接返回对象的边界。

完整源码zip10共127RDBC/409Druid/21资源557项逐项解压核对SHA，zip SHA-256为564a16eb050ce9ca17c4e8c4a78d4164ede53a0361ed4651b4c6b71e67d8d8dd。十项源增改删差异、实际命令/环境/log、反例/负向源及Java固定原件逐字节镜像到drafts/task7-ref-standard-return1，summary绑定实际rlib/exe与全部源码。1167冻结输入变化0，crates下.rs新增/删除集合均为空。

本轮仅候选，未生产apply、正式Cargo/Clippy/fmt、旧全套测试夹具迁移、真实数据库或Java运行时差分认证。其余LOB/Array/XML旧Access端口、Struct旧DTO唯一归属、SQLData/类型映射全矩阵、Callable返回/Prepared driver全矩阵、真实async流式读取仍开放；不得把共享值句柄视为第二套标准接口或Task7全完成。Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。


## Array唯一标准值句柄与共享释放确认候选（2026-10-08）

固定OpenJDK8u Array.java来源SHA核对。删除旧RdbcArrayAccess及factory/context/Druid重导出、废弃ResourceType::Array；ResourceHandleFactory::array现在接受Box<dyn Array>，共享值容器只持有Send标准对象、身份、能力和释放确认状态，不增加第二个驱动端口或RDBC后台释放调度。原始ResultSet保持原样返回，Druid资源监管仍由既有registry/coordinator承担。

初始命令模板误用-o与夹具unwrap_err隐含Debug错误原日志保留，不计产品缺陷；修正夹具后旧完整库同源RED仅E0308旧Access入参。完整128源RDBC严格-Dwarnings/forbidunsafe编译0；完整409源Druid旧重导出E0432 RED，移除后严格编译0。原58项消费者源逐字保留，新增七合同后65/0，追加驱动入口计数锁交接和panic合同后67/0。

全部32种READ/RANGE/TYPE_MAP/RESULT_SET/FREE能力组合验证十方法、FREE门禁及不支持零驱动调用；支持释放的16组合另验证Freed状态十方法零驱动调用。克隆身份、别名、SQL NULL、显式映射、1-based范围、原始游标独立关闭、原错误与free失败重试通过。原始free取消/panic后不误报成功、不恢复读取，后续free重试确认；并发克隆free只调用driver一次；取消读取不消耗资源。实际池化Array装入值句柄后，free调用者取消与父connection关闭仍由Druid监管链完成，重试加入既有释放，底层free一次。

独立完整源码反例分别仅移除能力拒绝或取得锁后的状态重查：两组128RDBC/409Druid严格编译0、同一消费者编译0、精确合同均0PASS1FAIL。后者通过驱动入口计数证明不能靠驱动自身closed错误掩盖句柄漏检查。两个负向编译确认Cell驱动不满足Sync（E0277）、旧ArrayAccess不可导入（E0432）。

八项源码增改删补丁与完整zip11归档，558源码/资源项逐项解压SHA核验，zip SHA-256 a1f32ec2296e9be01ef34050d9f1bf4c65ca4748286bbe7b4ed7dde9bb106555。实际命令、日志、Java来源、消费者、反例及产物SHA逐字节镜像。1167正式冻结输入变化0，crates下rs集合无新增删除。

本轮仅候选，未生产apply、正式Cargo/Clippy/fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。其余LOB/XML旧Access、Struct旧DTO归属、结果集返回/参数完整矩阵与真实async streaming仍开放；Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。


## Blob唯一标准值句柄、别名搜索及准入优先级候选（2026-10-08）

固定OpenJDK8u Blob.java来源SHA核对，保留十一项JDBC操作、1-based资源位置/0-based输入offset、原流与释放语义。删除旧RdbcBlobAccess、旧factory/context/Druid重导出及ResourceType::Blob；工厂接受Box<dyn Blob>，值句柄复用标准共享容器，不新增RDBC后台释放调度。

现有候选的position_blob(&mut dyn Blob)无法安全表示Java自身模式。新增BlobPattern是原模式参数的Rust别名表示而非第二套驱动端口：SameResource由真实驱动处理自身内容，Other独占借用另一个标准Blob；没有强制内容快照或固定搜索结果。两个共享句柄按稳定身份顺序取锁，Druid代理原样转发参数。此为候选公共签名变化，尚需正式API/SemVer门禁，不能声明兼容性已认证。

旧完整库同源RED为缺少BlobPattern及旧Access工厂E0308；完整128RDBC严格-Dwarnings/forbidunsafe编译0。完整409Druid旧重导出/旧参数边界RED后更新两源，严格编译0。仅迁移旧Blob驱动夹具的参数表示，原67测试函数体逐字比较一致；新增九合同76/0、真实占锁反向搜索77/0、池化别名与取消双锁79/0。归档器初始regex分组为65块（两处缩进测试合并），计数断言失败；保留原脚本并改用正式Rust遮罩及配对括号逐函数确认67项，不改测试或验收分母。128能力组合×十操作，支持FREE的64组合再验证Freed十操作零驱动调用；流原身份、范围/写入offset、失败/取消/panic重试、并发free一次及池化父关闭一次通过。

修复前的完整候选另有三个完整单源反例：移除length能力门禁、将自身模式固定返回1、改成总是先锁目标；各128RDBC/409Druid及同一77测试消费者编译0，精确合同均0PASS1FAIL。锁序反例在两个真实读取先占锁后使搜索排队，避免单线程无阻塞取锁掩盖死锁。Cell驱动Sync负向E0277、旧BlobAccess导入E0432。三反例绑定的是优先级修复前源码，未冒充修复后源码的重新执行。

随后补证目标不支持SEARCH且模式已关闭的错误优先级，原完整候选实际精确0/1 RED；独立后继候选先检查目标准入，再按原稳定序取锁并保留锁后重查。仅rdbc_blob.rs与standard_resource_value.rs两源变化（含格式化），Druid源全同；完整128/409严格重建0，同一全部80消费者80/0。保留原79源前缀及原失败记录，不以锁顺序改变旧目标准入错误语义。

十项最终源码增改删补丁与完整zip12归档，558源码/资源项逐项解压SHA核验，zip SHA-256 4cd9d7f07bfb92b97219c17caf2a170a5e3c4991e918ff4ffd58fce00d80d803。原始/后继完整manifest、修复前两源码、实际命令/日志/Java/反例与产物及extern SHA逐字节镜像。1167正式冻结输入变化0，crates rs集合无新增删除。

本轮仅候选，未生产apply、正式Cargo/Clippy/全仓fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。Clob/NClob/XML旧Access、Struct旧DTO、资源返回/参数全矩阵与真实async streaming仍开放。完整C1/C2已终态通过；原全仓内存/页面文件构建失败保留，-j1完整重跑另记，不与候选80/0混淆。Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。


## Clob/NClob唯一标准值句柄、共享视图及UTF-16候选（2026-10-08）

固定OpenJDK8u Clob/NClob来源SHA核对，保留十三JDBC操作；NClob仅继承Clob，不增加方法。工厂接管Box标准对象，退役两旧Access、context工厂与kind重导出。NClob上转不复制驱动对象，Clob视图/克隆共享身份、能力与释放确认；诊断保留NClob名称。复用既有Send共享容器，无新增RDBC调度。ClobPattern安全表示自身/另一标准模式，目标准入在双锁排序前检查；不强制快照，驱动处理真实搜索。新参数是候选公共签名变更，API/SemVer未认证。

旧完整库实际E0308/缺ClobPattern RED；完整127RDBC严格编译0，完整409Druid旧重导出/旧参数实际RED后严格编译0。首次消费者夹具ASCII collect类型推断E0283保留原source/log，显式Vec修复，不改变生产或断言。旧80项测试逐函数核对：仅两处模式参数包装为Other，归一化后全部原函数体逐字一致，断言不变；驱动夹具新增SameResource表示。新增九合同89/0，再补真实阻塞首锁反向搜索90/0。Clob/NClob各128能力组合覆盖十二非free方法与FREE门禁，释放后零驱动调用。UTF-16代理对/孤立代理项、1-based资源位置/0-based输入offset、Reader/Writer/ASCII原身份和实际写入、国家字符共享视图、自身start、错误优先级、取消/panic/失败重试、并发free一次、实际池化父关闭driver一次通过。

Cell Sync及旧两Access导入负向编译实际E0277/E0432。三个完整单源反例分别绕过length能力、固定自身搜索1、总是目标先取锁：每个127RDBC/409Druid及相同90消费者完整编译0，精确运行均0PASS1FAIL，唯一差异rdbc_clob.rs。另补按UUID实际排序确定两种准入优先级顺序，并直接调用NClob全部继承公开方法、原流身份及池化Clob/NClob交叉搜索，正常同源码扩展92/0；反例范围为90消费者，不冒充92重跑。源/日志/实际命令/env/rlib/exe/extern与固定Java来源SHA归档，十二源增改删补丁、完整zip13共557源码/资源项逐项解压核验，SHA eb508aad11dd7acd4b8195fc49144362a366f24bdf7cf45b65cbbb93ced057ac。

1167正式冻结输入及crates rs集合变化0。候选未生产apply、正式Cargo/Clippy/全仓fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。SQLXML旧Access、Struct旧DTO、资源返回/参数全矩阵与真正async streaming仍开放。完整Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN；全仓串行运行另计，不与候选90/0混淆。


## SQLXML唯一标准值与一次性读写候选（2026-10-08）

固定OpenJDK SQLXML.java源码SHA核对。工厂接受Box<dyn SqlXml>，共享句柄复用既有Send独占容器；新增内部SqlXmlValue只保存驱动与读写概念状态，不新增驱动接口、owner或调度。四种读取共享一次机会，四种写入共享一次机会；能力拒绝及未取得驱动锁的取消不消费，真正进入驱动前即消费，错误、panic或取消不恢复。getter别名共享同一状态。Source/Result的None保留Java null厂商默认请求，UTF-16及原流身份不改写。退役最后SQLXML Access与四个无调用方旧资源上下文类型。

旧完整消费者与Druid旧重导出实际RED；完整123RDBC/409Druid严格编译0，原92项测试源码前缀及函数体逐字保留，新增十合同102/0。再补真实锁队列中free先于读取的锁后状态重查合同，同源103/0。覆盖16能力组合/八操作、共享读写机会、原始流写入与UTF-16、nullable框架、失败/panic/取消、并发读只进入一次、释放失败及取消后重试、实际池化父关闭与fatal分类。Send非Sync及旧Access导入负向编译E0277/E0432。

三个完整单源反例分别绕过文本读取能力、重新开放一次性机会、把读取消费移到驱动await之后；每个完整123RDBC/409Druid与同一103消费者严格编译0，精确合同均0PASS1FAIL。实际命令、日志、源码及rlib/exe/extern SHA归档。十项增改删补丁、完整zip14共553源码/资源逐项解压SHA核验，SHA 3ce09e2565d5f38e0251790ec3bb325213c26dc344a4fe7894c4c639f5bb89e5。

1167正式冻结输入及crates rs集合变化0。仅候选，未生产apply、正式Cargo/Clippy/全仓fmt、API/SemVer、真实DB或Java运行时差分认证。源码复核另发现DruidSqlXml的cleanup_requested拒绝分支误用Blob诊断名称，当前103合同未独立覆盖该分支，待后继修复，不宣称该缺口已闭合。Struct旧DTO归属、返回/参数完整矩阵、ResultSet统一及真正async streaming仍开放。全仓原串行作业独立等待终态；Task6 FIX_REQUIRED、Task7八义务、Task8 INCOMPLETE、18顶层OPEN。


## SQLXML排队清理诊断修复候选（2026-10-08）

通过实际DruidPooledConnection创建两个标准XML。第一个真实驱动free阻塞同一registry coordinator，第二个free显式poll进入Pending后取消调用方；第二个驱动尚未free、读取零驱动调用，清理仍由registry持有。第二个XML读取实际返回Blob关闭诊断，旧完整候选精确0PASS1FAIL RED。测试释放两驱动、关闭父连接并确认每个driver free一次及owner归还一次后再检查诊断，不以挂起/超时冒充目标失败。

后继完整409Druid候选仅druid_sql_xml.rs一处字符串Blob改SQLXML，RDBC123源码及全部资源不变，原103消费者源字节前缀保留。完整库与同一104消费者严格编译0、全部104/0 GREEN，修复对应已观察分支。此前三反例仍绑定修复前候选103范围，不声称已对104重跑；该后继仅诊断修复。原RED/新GREEN、完整源码/产物/extern SHA、补丁及zip15归档，553源码资源逐项解压SHA校验，SHA 40cd443055c6b3b6d07774aaf7c5650472cf65acfeb3eba8365bc543dbd4173b。

1167正式冻结输入及crates rs集合变化0。未生产apply、正式Cargo/Clippy/全仓fmt、API/SemVer或真实DB认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。全仓串行原作业状态另记。


2026-10-08 完整Wrapper与缓冲命名候选：沿用唯一资源标准123RDBC/409Druid完整源码及精确Cargo依赖身份，三crate all-features严格rlib编译0；六原SharedResultSet合同源前缀逐字保留，加真实SQLite行读取/滚动/关闭，共7/0。SQLx准确buffered命名三文件迁移，无新增public API，不冒充真正streaming；初次Debug夹具与传递依赖搜索路径设置错误均保留。完整zip16 SHA 5bc700ba6865bdaeed310beffdccfc1cde60a5ec529a1bdc2415856486d07a64及三源补丁/原命令日志/产物SHA归档，1167正式输入变化0。尚未生产apply/正式Cargo/API/DB认证；Wrapper旧Access单测夹具迁移继续执行，18顶层OPEN。


2026-10-08 Wrapper完整单测唯一资源迁移候选：实际旧Access及旧工厂签名编译RED保留，五cfg(test)文件迁移标准Blob/Clob/NClob与独占Box；实际ExternalLease转移预分配非零大小原对象，三旧句柄相等观察映射为三个原对象地址相等观察，其他BASE断言保留并补三个长度观察。全features完整Wrapper test库严格编译0，全部16/0。中间canonical import设置错误日志保留，未新增第二端口或放宽断言。完整zip17 SHA d9daac8968a75b52c6fe4ac63e5750ea0f319f63535729942efe4b2632a7742b/五源补丁/命令与产物SHA归档，1167正式输入变化0。未生产apply/正式Cargo/API/DB/真streaming认证，18顶层OPEN。


2026-10-08 原资源测试候选迁移：Blob2/0、Clob2/0、资源上下文2/0，Callable原9测试及新增别名合同共10/0，均实际严格编译/运行。Callable原矩阵逐字前缀保留，旧索引/名称OUT误共享SQLXML导致两项HY010失败；独立OUT值修复前新增合同实际RED，修复后GREEN，同一selector仍共享身份/读取/释放状态。初次新增测试误用RdbcError.sql_state的E0599编译错误日志保留，经sql_exception访问修正。仅F候选，未生产apply或正式Cargo全仓；旧引用文件实际19个，之前20为计数错误，当前9个候选已迁移（不是全部验收）。源码/日志/脚本及SHA归档task7-legacy-resource-tests-migration4.zip。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 扩展类型候选实际推进：原Debug断言0/2 RED，StandardResourceValue恢复Open/Freed文字并准确区分释放中/未确认；原Array/Ref随后1/0、SQLXML重复读取实际HY010。分离独立表示场景并先free，保留原值/UTF16/Debug/身份/free断言，增加同一别名一次性读取合同，最终扩展3/0；完整三crate严格normal库编译0，中间缺JSON复制设置错误保留。已归档task7-extended-debug-verified-scope6.zip及summary。共享资源104回归原7305 LIVE（实际PID25100停首Clob清理），Wrapper单测原26114及trace原55404在编译；不计作通过，完整zip18尚未创建。旧引用19文件中10个候选迁移，余9个，未正式apply/Cargo/API/DB/streaming认证；18顶层OPEN。


2026-10-08 扩展类型与Debug候选恢复：原扩展两测试实际0/2 RED，唯一标准容器丢失state: Open；保留原断言，容器Debug恢复Open/Freed并区分Releasing/ReleaseUnconfirmed，不改状态机。完整123RDBC/409Druid/Wrapper严格编译0；中间复制忽略JSON导致manifest缺失的设置错误日志保留，补齐原assets逐项SHA。后继原Array/Ref通过、XML多次读取实际HY010，分离各表示场景对象并先free，原值/UTF16/Debug/身份/free断言保留，增加同一别名一次性合同，最终3/0。共享改动完整资源消费者104/0、完整Wrapper单测16/0。旧消费者链接不同Tokio产物（205f/6f）导致清理任务no reactor panic并挂起；30秒诊断副本明确阶段，核验原PID25100路径后终止该错误设置运行，不计业务FAIL。后继绑定当前完整库精确依赖身份再验证。完整zip18源686项逐项解压SHA验证，仅standard_resource_value.rs不同于zip17；未生产apply或正式Cargo/API/DB/streaming认证。旧引用19文件中10个候选已迁移，剩余9个（Wrapper8及Druid外部消费者1）。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 Wrapper旧资源测试迁移推进：8个原文件在新完整库下实际编译RED（边界E0405、七个深分支E0432）；边界文件迁移为六种公共值句柄身份断言，完整all-features边界4/0、深分支part01为8/0。七个深分支复制夹具迁移原73131继续，尚无全套终态，不计作18/19完成。外部消费者完整F候选workspace已按zip18叠加686源码资源，原测试编译0；首次嵌套Cargo因超长输出路径LNK1104失败，属于设置错误不是语义RED。原58816在后继验证；独立原嵌入消费者以短F路径实际Cargo运行原72492，等待完整依赖构建和真实旧Access诊断。未生产apply/安装/提交/安全修改，全部18顶层OPEN；机器状态task7-wrapper-external-migration-progress4.summary.json。


2026-10-08 候选归档源下限修复：只读对比发现zip18遗漏正式Druid池寿命cfg(test)入口及三个已接受测试源文件，若直接apply将丢失原BASE测试。固定入口完整差异及测试身份后恢复原源码，zip19共689项逐项解压SHA核验，其余685项原manifest来源不变；四个恢复文件与正式源码逐字节一致。该源下限RED/GREEN不是运行验收，尚未重建候选Druid单测/Cargo/C1/C2；未改正式Rust源码。机器证据task6-e-space-task7-candidate19-preserve-unit-floor.summary.json。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 Wrapper/外部消费者原作业续验：当前Wrapper已终态通过的范围由task7-wrapper-external-migration-progress5.summary.json逐日志SHA及结果核对，后继原73131不重启；外部原短路径Cargo实际exit101/E0405旧RdbcArrayAccess等14诊断，首次长路径LNK1104不计语义RED。迁移后三项原测试以短TMP完整重跑原7641，私有类型保护及错误身份已通过，公共合同/SQLite仍待终态。独立XML能力回归首次E0308测试设置错误保留，修正RdbcString比较后原58645重跑，尚不计GREEN。候选源核对686项无漂移但14旧叠加文件中3项为必须保留的寿命测试；zip19已恢复入口及三源，不能删除它们。未修改正式Rust或安全策略，18顶层OPEN。


2026-10-08 候选集成准备：已保留原104项资源合同源全文并新增计划要求的resource_owner_cancellation_test.rs候选入口；源SHA及原104通过日志绑定归档，尚未正式Cargo认证。集成脚本语法编译通过，设置三个原作业终态/成功前置，恢复zip19的689源、隔离精确11退役文件、保留三寿命测试源，迁移原测试及XML能力后依次完整Druid lib/owner合同/RDBC/Wrapper/Druid Cargo测试。尚未执行集成，防止改变原Cargo使用中的候选workspace。原73131/7641/58645均经同句柄45秒轮询确认LIVE，不能按观察超时重启或推定结果；18顶层OPEN。机器证据task6-e-space-task7-integration6-task7-integration-preparation6.summary.json。


2026-10-08 Wrapper八原文件终态及独立RDBC合同：原73131实际exit0，边界4及深分支8/11/21/12/9/14/5，共84PASS0FAIL（此前口头89为加总错误，各原日志不变）；八源SHA、全部日志SHA、七个归一化原测试后缀及正式原源未漂移均核验。新增resource_standard_contract_test.rs候选以RDBC公开归属读取原Blob/Clob/context合同并保留Ref/Struct七项，严格编译无Druid extern、运行13/0。首次提取缺模块闭合符设置错误保留，修复后原Ref测试前缀不变；Array/SQLXML独立完整矩阵未计完成。Owner104候选及最新集成脚本一起归档evidence7，逐项解压SHA核验。XML能力GREEN及外部三原测试终态待验，集成未执行，未正式apply/Cargo，18顶层OPEN。


2026-10-08 外部消费者完整终态：原7641实际exit0，全部三项原测试3/0，保留错误身份、独立Cargo公共API与六种资源句柄/工厂身份、私有PreparedStatementKey/MethodType不可访问探针及真实SQLite操作。迁移源终态SHA不变，完整日志SHA4f9163676ec8916cb1d27e6fb70a34097a83b89ce263b830e031c1803cf71ecc，源/日志/summary逐字节镜像。之前长路径1/2 LNK1104设置失败与短路径旧Access14诊断RED原件保留，不以环境失败冒充语义RED。本次仅F候选范围，尚未候选完整Cargo/C1/C2或正式apply；XML能力修正原58645仍待GREEN，18顶层OPEN。


2026-10-08 XML能力及候选正式Cargo启动：原58645实际exit0；新增能力回归在旧READ|FREE上实际8/1 RED，FREE-only修复后完整9/0 GREEN，保留原raw字符串与Source不支持行为。外部原7641三项3/0后，候选集成原60792已实际启动：恢复zip19的689源，精确隔离11退役文件（不删除），保留寿命入口及三测试源；迁入19原文件与已有外部消费者，七个XML夹具同步收紧能力，另纳入owner104和独立RDBC13候选共六文件。完整Druid lib all-features Cargo正在构建，后续owner/RDBC/Wrapper/Druid全套由同runner顺序执行，尚无通过终态。全F workspace输入逐项SHA冻结到integrated6-frozen-inputs9.json，等待终态核对；未正式apply/安装/提交/改安全，18顶层OPEN。


2026-10-08 独立RDBC资源合同扩展：保留原13项源码字节，完整提取Array/SQLXML独立矩阵，新增8/10项；严格编译exit0、实际31PASS/0FAIL/0ignored。两项依赖Druid的池化场景仍保留在原owner104目标，没有删除断言。首次提取残留Duration未使用导入的设置失败日志保留，仅删该导入后GREEN；无Druid/Wrapper extern。负向探针实际E0277拒绝强加Sync、E0432拒绝旧Access。证据zip11共20项逐项解压SHA核验，SHA 69f5deac14f0f7d2fd7fdb7ed6fe697d057f57531096cd29ad789abc6a95a487。原60792仍LIVE，1147冻结输入及crate新增rs变化0，未改运行workspace；正式Cargo认证/生产apply仍待完成。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 独立RDBC正式Cargo准备：从集成1147冻结输入复制独立workspace，纳入31合同，不改原60792运行输入。首次仅固定Cargo未固定rustc，代理自动同步stable并安装rust-src；确切Cargo52812/子rustc43672路径与父PID核验后终止，原日志保留，属于工具设置失误不是语义RED。stable manifest-rust-src实际mtime与SHA记入setup14；不擅自卸载共享组件。原Cargo12终态非0且输入变化0。重试13显式固定现有Rust1.97.1 rustc/rustdoc/toolchain并禁止auto-install，使用独立target，原31842已确认LIVE；尚无正式Cargo31终态。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。


2026-10-08 候选正式Cargo局部终态：固定Rust1.97.1与禁用auto-install后，独立RDBC resource_standard_contract_test all-features/locked/offline/-j1实际exit0，31PASS/0FAIL，终态冻结输入变化0，日志SHA 4db452a4df487cf3821f635e224a993dc2840a3252504711efe92cd325c9c14e。该源仍有两条自定义cfg与两条XML夹具字段dead_code警告，不能记严格质量门禁PASS。原集成60792的Druid all-features lib已实际exit0/153PASS0FAIL，保留的池寿命测试通过；后继owner104仍待终态，全RDBC/Wrapper/Druid后继不能预计通过。1147集成冻结输入复核变化0，owner自定义cfg及未使用辅助代码警告已清点，须冻结作业结束后修复并验严格门禁。未正式apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。证据task6-e-space-task7-cargo-pass15-task7-cargo-pass15.summary.json。


2026-10-08 owner104正式Cargo终态：原60792的resource_owner_cancellation_test all-features/locked/offline/-j1实际exit0，104PASS/0FAIL/0ignored；日志SHA 995efb905d10e80fcd7ad4631fdbc01c234398bf2f9171b5a771c7e184adc3c7。原153单测通过保留，1147集成冻结输入复核变化0。完整RDBC/Wrapper/Druid后继继续原作业，不重启、不预计通过；owner测试警告仍须修复和严格质量认证。独立31合同已另有Cargo通过，尚未纳入该冻结集成workspace，不能混淆为同一快照。未正式apply，18顶层任务继续OPEN。


2026-10-08 原集成终态与续跑：原60792实际exit1，Druidlib153/0与owner104/0后，完整RDBC因生成StrictConnection三个旧资源返回签名实际E0053/Cargo101；原summary的terminal=false是runner断言后未更新，terminal17审计更正为已停止，后继Wrapper/Druid未运行。原1147冻结变化0。候选生成器从当前standard源码重新生成，仅common增加Blob/Clob/NClob导入、Connection三返回改Boxdyn；全部生成行为体/选择器归一化比较保留，原正式生成器未修改。将已Cargo通过31合同纳入候选，新1149输入冻结；原37404从失败的完整RDBC门禁续跑，再Wrapper/Druid，未重跑已通过两目标。当前进程已确认LIVE、1149冻结复核变化0；新完整C1/C2/方法账本签名/API与严格质量尚未认证。未生产apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。


2026-10-08 资源候选质量推进：独立RDBC31严格rustc原4警告实际RED；声明两个真实负向cfg而不放宽unknown cfg，XML不透明夹具改等价字段Debug后严格编译/31运行GREEN。严格Clippy先实际报标准容器回调type_complexity，私有生命周期回调别名修复，不加allow；后续六条测试诊断以等价assert panic注入、XML/Array rustfmt及新外层Ref模块改名修复，原内层七合同源码不变。最终严格Clippy exit0/零警告，完整独立合同Cargo31/0/零警告。证据zip24逐项解压SHA核验，SHA b30d764286737965011cb1548441e4c09cfb6c07c63939b57e0818e4a61e507c。完整集成17实际{'passed': 504, 'failed': 6, 'ignored': 0, 'blocks': 20}，两个外部目标LNK1181/1104，未验证到预期语义诊断；生成435已通过。原37404终态exit101且1149输入不变。停止后集成23合入已验证质量修复及两外部helper短target-root配置，测试体逐字保留；新1149冻结变化0，原40390完整RDBC/Wrapper/Druid续验已确认LIVE。owner质量/API/C1/C2/全仓最终认证仍开放，未正式apply；18顶层OPEN。


2026-10-08 owner严格质量与完整RDBC候选终态：原70931实际exit1，strict-green直接rustc漏声明cfg(test)导致105宏拒绝；原RED另含真实unused/customcfg诊断。runner25收据command列表引用发生后续变更，terminal-audit29明确更正实际RED命令且原日志不动。补cfg(test)后相同源码严格rustc exit0、105PASS0FAIL；保留原104全文仅等价XML Debug变更，新增Send正向合同1项。七原负向探针全部真实E0277/E0432拒绝。严格105仅coherent旧别名之前库族，非final源码Cargo认证；独立final候选Cargo Clippy原35978仍待终态。原集成40390完整RDBC实际exit0/510PASS0FAIL/20块，1149冻结输入变化0，Wrapper/Druid后继尚未完整认证。证据zip29 SHA 82b414a076ce4d6872c4372e0a1c9548bff8e6d4183fedd647ec3d0e52952d1a，逐项解压SHA核验，未正式apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 owner证据命令更正：audit29根据retry26重建原RED时输出路径误留owner26.exe；实际原70931 RED输出为owner25.exe，补充task6-e-space-task7-owner30-task7-owner25-command-correction30.summary.json记录精确命令。原zip29/失败日志不改写；来源路径、库族和cfg()其余部分一致，不影响105运行或七负向探针结果。


2026-10-08 owner最终候选严格Clippy通过：原35978实际exit101/库5诊断，四冗余闭包改等价方法引用、Array注释归位，原96432继而actual101/测试8诊断。保留105身份/所有原断言，复杂Struct入参精确别名、两显式drop改词法作用域、五panic注入等价assert，不加allow。原15012的正式Cargo Clippy target实际exit0/零警告，最终候选Druid/RDBC源码库族；原owner105 Cargo运行随后启动仍待终态，后继RDBC31亦不能推定通过。质量zip33 SHA ac4bd3741a7b7b674447bd5130beed3df48b1debe1539a4e1f78ec321ad80e26，逐项解压SHA核验，六源码及root manifest摘要绑定；主集成40390仍原冻结输入运行，未合入本次独立质量修复。未正式apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 Task7流式实现调度决定：Ruling：ForwardOnly且正fetch_size通过既有标准Statement/PreparedStatement入口选择真实需求驱动有界游标；零hint/滚动查询及generic execute保留原缓冲行数语义。ResultSet仍唯一canonical，Stream仅拥有游标的扩展，不新增driver端口。依据ADR-RDBC-006 Task7；若错误将影响既有fetch hint语义，须完整BASE/参数/生成键/取消回归，八项义务不缩减。独立workspace35从主集成1149冻结及quality33源码建立，新增async_result_set_test.rs五真实SQLite合同；先观察原12642基线RED，未修改生产或两个运行workspace。


2026-10-08 owner最终候选正式Cargo终态：原15012实际exit0，严格Clippy零警告、owner105PASS0FAIL及RDBC资源31PASS0FAIL均完成；六源码及root manifest与quality33绑定摘要完全一致。zip37 SHA 606354df3fd0601cab1ecfa5236655aab7ccb8291fc48a63fe2ea2cbe03bc497逐项解压核验，原27/31失败与32修复链保留。只证明独立最终候选上述范围，主40390完整Wrapper/Druid尚待终态，后续合并仍须fresh全源门禁，未生产apply。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 Task7真实按需SQLx游标候选：原Cargo39实际2PASS/3FAIL，明确eager查询提前报后段错误、known_row_count提前Some；保留34夹具类型错误与36原生链接失败，不计语义RED。仅独立workspace35新增容量1请求/保留pending响应、同物理连接SQLx fetch流、父关闭先通知游标再取锁；零hint/滚动/generic execute保持缓冲，准备生成键保留原路径。原Cargo41实际exit0/5PASS0FAIL、冻结输入变化0。Stream扩展外部metadata探针43实际E0599 RED后，候选新增唯一ResultSetExt owning Stream、5标准Stream合同及取消/BASE/真实SQLite Stream补充（总10）；仅增加已锁定futures到RDBC清单/lock依赖边，不安装或更新版本。原98773正式Cargo45继续验证新候选，尚无终态，不能扩大5项GREEN到新源。zip47逐项解压SHA核验，SHA 99cb83086fd3f99a8f82e7020208c69530f40e92d6ae07cc9cff53663afee8f2；正式Rust未apply，完整主40390未重启，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 Task7流式生命周期推进：Cargo45的扩展SQLite因夹具枚举CloseCursorsAtCommit不存在实际E0599；只改实际CloseAtCommit保留原10身份，补取消setup/待处理next Drop/Prepared generic缓冲，共13SQLite合同在原51实际13/0。生产请求调度器轮询合同1/0，无请求0次、每次请求一行；RDBC Stream五项5/0，RDBC严格Clippy0。Wrapper严格Clippy先一分号诊断，原54修正后实际0。上述仅各冻结候选范围。进一步故障注入54实际0/1 RED，worker失败后重复close触发JoinHandle二次轮询panic；语句owner补充56实际13/2 RED，close/Drop未使流式子游标关闭。仅F候选57修复完成句柄无论成败移走（保留等待取消安全），新增语句watch关闭/Drop监督；原62305 Cargo58正在对新源完整默认lib、15SQLite与严格Clippy续验，不扩大旧GREEN。主原40390的完整Wrapper all-features现已实际exit0/368PASS0FAIL/49块、1149冻结变化0；完整Druid继续原作业。zip59 SHA a58801a5a35ea9d60b55bcf0f1e7aaba624f7dfc3f57755bfb8dba079f8c0ea3逐项解压核验。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7流式候选生命周期最终局部终态：原62305/Cargo58实际exit0，完整默认Wrapper lib12PASS0FAIL（含生产调度器与故障worker重复关闭），真实SQLite15PASS0FAIL（含Statement close/Drop、取消next/close/setup及同连接临时表复用），RDBC/Wrapper两个指定目标严格Clippy均exit0，冻结输入变化0。RDBC Stream原五合同5/0，RDBC清单/lock/扩展/测试与原45冻结逐项一致；不得扩大为全wrapper/all-features质量认证。54原0/1 join panic RED与56原13/2语句owner RED完整保留，57修复不放宽断言。zip61 SHA 3cd5becffd16d1102d5c2a058acd622f34839638225cfaa86c5303464b68feda逐项解压核验，源码/命令/原失败/最终日志归档；正式Rust尚未apply。原1287/Cargo60已启动完整RDBC、四原BASE目标、Wrapper all-targets Clippy与fmt续验；主40390完整Druid仍独立继续，已通过的资源候选Wrapper368/0不扩大到流式新源。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7候选整合推进：原集成23终态RDBC510/0、Wrapper368/0，Druid未进入运行断言，因异常路径夹具三个旧资源工厂返回签名E0053/Cargo101停止。仅F候选改三签名，反向字节比较证明所有其他内容不变，原目标64实际Cargo exit0/1PASS0FAIL。新池化SQLite五合同65在旧eager实现66实际0/5 RED（提前物化/后段错误），原断言不改；原66终态后合并已验证质量与流式20路径到主候选，保留三个签名修复与五合同，原69正在验证，未推定GREEN。独立流式60的514/1失败为嵌套Cargo长路径LNK1181；只修验证短target环境后原63完整RDBC515/0，四BASE/严格Clippy/fmt后继尚待终态。冻结核查纠正：64/66仅Rust/manifest/JSON子集1089/1090，未包含原1149中的60 README/assets/proto/cert输入；67已逐字节核验这60项在两workspace与原快照一致，69纳回完整原分母及新源，不降低验收范围。zip70 SHA 36992d46e597b86fff8eb6aca778f621a9a3079031d5a9ae4a25ddeeda168ae6，61项逐项解压SHA核验。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7池化流式RED/GREEN终态：原69实际exit0/五合同5PASS0FAIL，与原66的0/5使用相同完整断言；1156原完整分母加新源冻结变化0。证明父连接关闭与真实临时表/单物理连接复用、Statement关闭、前199行/后段错误、Prepared缓存命中后流式、取消next不丢首行；不扩大为全部DB或最终Task7验收。独立原63实际完整RDBC515/0、四原BASE目标46/0、Wrapper默认all-targets严格Clippy0；最后cargo fmt因Windows命令行长度os206退出，尚未比较源码。短junction71仍canonicalize回长目录失败；独立短物理目录F:/codex_build_cache/d72逐字节复制完整1156文件后，72原fmt实际到达比较并发现格式差异；官方cargo fmt修复21.rs，所有函数身份保留，73完整fmt check exit0，其他候选不改。完整1156最终候选源及上述终态日志zip75 SHA f479060b4d099ee978025edea5d3436f5da8464c270e7309f853297f571174dc逐项解压核验，证据25项。新原74/session57659正在同一最终短路径快照续验完整RDBC/Wrapper/Druid allfeatures+owner/池化严格Clippy+fmt，未推定通过；原完整Cargo/外部API/C1/C2及八义务最终认证仍需闭合。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7转发与owning Stream释放推进：独立RDBC原77正式Cargo/allfeatures实际6PASS0FAIL，原Druid完整SharedResultSet合同逐字保留，仅复制到纯RDBC入口；无Druid/Wrapper依赖，不据此关闭最终转发/API义务。新增真实池化Stream提前Drop合同，最初futures测试导入E0432为设置错误，std poll_fn修正后原80实际0/1行为RED；补普通线程Drop后原82实际0/2，均同连接后继查询超时且Statement仍存活。仅F候选83新增唯一ResultSet的release_owned默认Drop及私有Stream所有权guard，池化覆盖提交原registry监管队列并捕获owner runtime；普通共享视图Drop语义不改。83初次文本插入匹配两个close失败，部分编辑归档后按82冻结SHA恢复六原文件，再限定trait位置重做；未把设置错误计语义RED。新增公开方法必须进入最终API/compiler/SemVer证据。原84/session81038冻结1158完整输入正在原112资源/池化合同、RDBC5、Druidlib与严格Clippy验证，尚无GREEN；原74/session57659继续独立冻结1156快照，已确认RDBC515/0，其余不推定。zip85 SHA 59c15406bbf24d5405260d88d8bc293bfc2f5cee5b0008ab90d51d1df72533da逐项解压核验，正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 owning Stream释放原84终态：真实池化/资源三个目标112PASS0FAIL（原Drop两项2/0、原owner105/0、原池化五项5/0）、RDBC Stream5/0、Druidlib153/0，原1158完整冻结输入变化0；与原82的真实0/2使用相同完整断言，普通线程Drop也通过。严格Clippy实际101：新增模块插入在原expect与mod result_set之间，导致module_inception及未履行expect两诊断，保留原失败。原84停止后88只调整声明位置恢复原expect附着、不加allow，补私有guard中文方法文档，保留原两Drop合同并增加未轮询/等待首行Drop两项；共三文件变化与84冻结逐项核验，其余不变。原89/session7430正在新1158快照执行两个严格Clippy及新增四Drop/原owner/原池化/RDBC5/Druidlib，不能扩大84结果。zip90 SHA 7ade414e0f8c5f0197322b46bb4ed07926381805abae63dca16359db3bf836b4逐项解压核验，证据16项；原74仍独立续验未改输入。正式Rust未apply，新增release_owned仍需最终API/compiler/SemVer证明；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7释放修复与方法账本推进：原89在88质量修复后的1158输入上，RDBC Stream指定严格Clippy与Druid三指定目标严格Clippy实际均exit0；原owner105/原池化五项/四Drop共114PASS0FAIL，四Drop含原两项及未轮询、等待首行，RDBC Stream五项5/0；各已完成命令冻结变化0。Druidlib后继尚待终态，不把84旧153/0扩大到新源码。另独立短目录d91逐字节复制相同1158源码/资源并补62原验证工具/账本，1220冻结；原91正式C2静态67文件/1283身份/1146EXACT/137参数化/0移除PASS，executed=no；正式C1静态实际RED为create_blob目标签名漂移。92只读确认Blob/Clob/NClob三行过期，91终态后93仅更新三行target_signature及SHA两列，全部479原行、BASE签名、选择器和其他列保留，反向canonical字节检查一致。原94/session39912在新1220冻结上完整C1静态续验，未推定GREEN；仍需完整runtime C1/C2/API。zip95 SHA 5053bc5e23e8fbea71f5cc9346fb01ebc1bfb5a41d7cf7d316b37267198e6679归档完整1220候选及已完成日志，逐项解压SHA核验。原74独立继续，正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 owning Stream释放完整局部终态：原89实际exit0，原owner105/原池化五项/四Drop合计114/0、RDBC Stream5/0、Druidlib153/0；两个指定严格Clippy目标均0，1158完整冻结变化0。四Drop覆盖未轮询、等待首行、首行后及普通线程，原82两失败断言保留。C1原94终态仍RED：三目标签名刷新后，原生成器模板缺资源trait import，common.inc.rs复现漂移；原1220冻结无变化。94停止后96只补Blob/Clob/NClob一个导入行，与已保存候选17生成器逐字一致；原--check实际14文件PASS，全部生成Rust源码/测试体/选择器变化0，不使用--write覆盖测试。新97/session63039固定现有Rust1.97.1并禁auto-install，在1220当前冻结上完整C1静态续验，未推定GREEN；候选相对zip95仅该生成器一文件变化。zip98 SHA 99627f7f25c2c8aa1a321890cbf4cc01ebc7a414b51d262ccf6d6ab9679626db归档完整终态日志及该精确delta，逐项解压核验。原74继续独立全套，正式Rust未apply；当前全C1/C2 runtime、格式整合、完整全features/API/compiler/SemVer/MSRV/features/review及Task7八义务仍开放，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task7最终候选整合：原97完整C1静态实际exit0，479映射/442选择器/完整C2来源检查，executed=no；1220冻结变化0，日志SHA 7111b986db253d9964372e1279eb79b0e7d6387be8b0805489be1fa1ae1f309e。97停止后99先归档完整1220输入并逐项核验，再验证五个SharedResultSet生产对象与已通过候选77字节一致，复制原纯RDBC六合同，原Druid合同不动；官方cargo fmt修改21个rs且保留全部函数身份，fmt check及原生成器--check14文件均exit0。新完整1221输入冻结；原100/session70615继续纯RDBC六合同实际Cargo、指定严格Clippy及正式完整C1 runtime（含完整C2），未预计通过。另原74独立旧1156快照完整RDBC515/0与Wrapper385/0均实际exit0/冻结变化0，后继Druid全套未取得终态，不扩大到新1221源码。zip101 SHA 750e6af4ad2f0becb24384e1b439cc67c9b9165c266165142ee2720720f0f750完整归档1221候选和终态证据，全部解压SHA逐项核验并逐字节镜像。Ruling: 保留原Druid行为合同并新增独立RDBC入口，不用跨产品依赖证明独立standard能力；格式化仅在97终态后执行，原74输入不动。正式Rust未apply；当前完整runtime/API/compiler/SemVer/MSRV/features/review及Task7八义务未关闭，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 最终1221候选纯RDBC转发实际通过：原100/session70615中task7_shared_result_set_contract_test allfeatures/locked/offline实际Cargo exit0/6PASS0FAIL，指定严格Clippy/-Dwarnings实际exit0。六项包含borrow排队取消、无Tokio运行时等待、共享游标/争用、关闭错误重试、active close取消恢复及完整RowSet getter/Stream合同；保留原Druid测试，未增加Druid/Wrapper依赖。原1221冻结逐项变化0，两原日志SHA已校验。完整C1实际runtime（含C2）后继原Python32664→45500已确认LIVE；原74Python61032→Cargo3876仍执行旧1156快照Druid完整回归，未重启、不推定终态。zip102 SHA ace16fab93881d191fd786ab954bc1773079b0a313bbe73c4646763ecb61d605五项证据逐项解压核验并镜像。该6/0只证明本目标；完整全features/C1/C2/API/compiler/SemVer/MSRV/features/review及Task7八义务仍开放，正式Rust未apply，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 当前候选公开入口编译诊断终态：原103/session17754在独立dr103_target缓存实际构建当前1221冻结RDBC allfeatures/lib/locked/offline，Cargo0；从原compiler-artifact JSON唯一rlib重跑全部68个原探针，原Java身份/target/shape/源字节/作用域逐项保留。实际54PASS/14FAIL，另1host项未认证，1221冻结与rlib终态SHA均无变化。剩余失败target：druid_rdbc::resource::SqlData、druid_rdbc::resource::SqlInput、druid_rdbc::resource::SqlOutput、druid_rdbc::data_source::ConnectionEvent、druid_rdbc::data_source::ConnectionEventListener、druid_rdbc::extensions::RowSet、druid_rdbc::extensions::RowSetEvent、druid_rdbc::extensions::RowSetInternal、druid_rdbc::extensions::RowSetListener、druid_rdbc::extensions::RowSetMetaData、druid_rdbc::extensions::RowSetReader、druid_rdbc::extensions::RowSetWriter、druid_rdbc::data_source::StatementEvent、druid_rdbc::data_source::StatementEventListener。只证明原import/trait-bound探针，不认证精确enum/module形态、Java行为、SemVer或host项，不能作为Task8完成。zip104 SHA 6b717d6bcd444d3f1becc64bb6f3790c131c0cc9f59277ec0d567d1ea3c0dea0全部探针/命令/诊断/JSON/冻结输入逐项解压SHA核验并镜像。完整C1/C2 runtime原100及旧快照完整Druid原74继续各自原作业，未改冻结源。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 标准事件候选与原完整回归终态：独立d105逐字复制1221候选，新五项真实事件合同105实际E0432 RED；新增四个一对象一文件的javax.sql标准事件/监听器，Arc保留原PooledConnection、PreparedStatement与SQLException引用身份，允许Java nullable statement/exception；不重导出同名Druid Filter事件，不引入产品依赖。原106实际5PASS0FAIL，严格Clippy因测试tuple复杂度RED；108仅精确ObservedEvent别名，不加allow、不改断言，最终5/0、严格Clippy0、fmt0，1226冻结变化0。原110使用新Cargo JSON唯一rlib重跑全部68原探针实际58PASS/10FAIL/另1host未认证，四事件入口由FAIL转PASS；余SqlData/Input/Output与七RowSet项，未认证精确kind、全Java语义或source-provider注册/Druid事件整合，不关闭Task8。另原74/session57659已完整终态：RDBC515/0、Wrapper385/0、Druid2951/1共164块，唯一失败task6_pool_maintenance_differential_test_remove_abandoned_reclaims_timed_out_borrowed_connection，新鲜借出扫描left1/right0；1156冻结无变化，后继strict/fmt未执行。原107保持原源码/断言/超时/完整10身份，完整目标复查实际9/1同一失败，根因尚未认证，不以重跑掩盖失败。zip109 SHA 34823d32312df40e36d0b05e2d9cef381d3564ad20b9f1b24c41f33c016da20b完整1226源、原RED/严格失败/最终GREEN/完整74终态/107失败及四份固定Java源码全部解压SHA逐项核验并镜像。原100完整C1/C2 runtime继续原1221冻结，不能扩大事件GREEN；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 removeAbandoned固定基线顺序修正候选：原74完整2951/1与107完整目标9/1保留真实RED。CodeGraph定位ActiveConnectionLease先Instant计时、后强制Backtrace符号解析，而固定Java基线33824c3dec1612711f9bb4e409319bcab2e4cd0e的DruidDataSource明确先getStackTrace/保存，再setConnectedTimeNano；git show原字节及顺序片段归档，不依赖当前Java分支推断。新独立d111从108完整1226源逐字复制，仅active_connection_lease.rs先完成同一原Backtrace字符串捕获再记录原Instant，原管理输出/弱令牌/超时值/扫描比较及全部测试断言不变，fmt后仅一源变化。Ruling: 对齐Java借出诊断与计时顺序，避免Rust符号解析成本消耗尚未交给调用者的租约预算；不通过延长超时或删断言获得GREEN。原111/session84147/Python16732已确认LIVE，正在原完整10身份目标，后继14维护语义与fmt尚待终态，不先宣称修复成功或全仓PASS。zip112 SHA 9cec2cbd5a14161750fa4f15cc81ba78cacaca64118f0371d94968cd6c1f2bb3为完整zip109父快照的一源精确delta，完整1226清单/Java原字节/修复前源/runner逐项解压SHA核验并镜像。原100/session70615仍独立1221冻结完整C1/C2，未改输入；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 removeAbandoned顺序修正原111完整局部终态：原session84147实际exit0，原完整维护差分10PASS0FAIL（含此前fresh borrow失败）、维护语义14PASS0FAIL、fmt check exit0；1226冻结逐项变化0。固定Java先捕获堆栈再记录借出时钟，一处active_connection_lease.rs修正，原测试/断言/超时均保留；107完整9/1 RED及74完整2951/1历史失败不删除、不冒充当前全仓结果。zip114 SHA 33e2d7659ffed914413a6bd7e83d53d144e89b827302775ced53b92705d1f983保存原RED、最终日志/命令/Java基线/一源delta，全部解压SHA及E镜像逐项核验，父完整源zip109可重建1226候选。正式Rust未apply。原113/session80161/Python41232已在同一1226快照启动完整workspace/allfeatures/locked/offline/no-fail-fast/-j1，后继严格alltargets Clippy、文档、fmt及正式完整C1含C2；单项失败仍继续收集后继门禁，输入漂移则停止，不以局部通过代替最终验收。原100仍独立1221冻结运行，未重启或更改输入；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 SQLData/Input/Output完整标准面TDD启动：固定OpenJDK 7373384725418fa645fb4508ec05a869bc7fc3e7三份原源码逐项匹配既有来源SHA，3+28+28共59方法，保留两个JDBC4.2 default。独立d115完整复制原111的1226输入，仅新增纯RDBC六合同，无Druid/Wrapper依赖，冻结1227变化0；六项覆盖全部reader/writer入口、SQL NULL、外部SQLData声明顺序、转换失败、typed默认拒绝无消费与原流游标/UTF16身份。原115/session16829实际Cargo101/E0432两组诊断：三个标准trait和两个具体缓冲对象缺失，为预期公开面RED，六项行为断言未执行、无GREEN、无生产实现。执行简报保留非null资源/SQLXML写状态/custom mapping/type map/错误传播及旧Druid标准树迁移等后续义务，不把六合同扩大为59方法完整语义认证。zip116 SHA 08b4e9d78433d60d1e8bdd766b0d5b9e2da2c36d5b58c237f69726b0d6918480保存测试delta、完整Java方法清单/原件、命令/原RED日志及完整冻结清单，全部解压SHA与E镜像核验，原111源码不改。原113完整workspace与原100完整C1/C2各自原进程继续运行；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 SQL映射标准候选与非Sync描述符修复终态：固定Java三标准3+28+28共59方法，独立d117新增七个真实生产对象、两mod导出及原完整合同，SQL NULL、声明顺序、转换错误、嵌套UDT、流身份与失败无部分写入均有实际合同。117/118公开面E0432 RED、119夹具Cell.set(await)编译错误、120仅改局部赋值后的10/0及RDBC536/0保留，不把夹具错误当产品语义RED。新增第十一合同123实际E0277 RED证明typed输出错误要求SQLType Sync；124仅sql_output.rs改对象安全boxed Send Future，Future仅借用self、不持有描述符，原11测试字节不改，实际11/0后严格Clippy因测试drop_non_drop失败。127仅以词法作用域替换显式drop，保留原非Sync Cell、Send Future约束和全部断言，生产源码与124不变；最终正式Cargo合同11/0、指定严格Clippy0、完整RDBC allfeatures537/0、fmt0，1234完整冻结变化0。125从新Cargo JSON唯一rlib重跑全部原68探针实际61PASS7FAIL/另1host未认证，剩七RowSet项；不认证精确kind、全Java行为或SemVer。custom type map实例化、SQLXML pending writer/校验、Druid旧标准树迁移及最终全仓/API/实库仍开放。zip126 SHA b9988c511d3f06ed8dfc6f6f8eaab14520ca118c7814bc6fa9cc0ac2e0da63d5归档原RED/夹具与质量失败/最终GREEN/Java原件/探针与1234清单，相对完整zip109为11路径delta，逐项解压SHA与E镜像核验。更正原100/session70615：实际完整C1 exit1，子C2 Wrapper allfeatures lib命令1800秒超时、replacement NOT_RUN/ERROR；原1221输入不变，历史C1 PASS不得扩大到该候选。113完整候选回归尚未取得终态；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 当前SQL候选完整验证接续：129独立短目录逐项复制127最终1234完整输入，所有SHA一致；原session70876/Python52828及直属Cargo实际存活。先执行与原C2完全相同Wrapper allfeatures lib/show-output命令完成原生预构建，再执行完整workspace/allfeatures/no-fail-fast、严格alltargets Clippy、文档、fmt与正式完整C1含C2；原验证器超时/分母/测试断言不改，单项失败继续收集门禁、输入漂移停止。当前实际编译libduckdb-sys，尚无运行终态或PASS；100原1800秒超时保留。zip130 SHA 529af4a9c800ea4ca57335a554b3700251615423664d3983f5b4104e1ae7f1d9保存runner、1234清单及真实进程观察，逐项解压与E镜像SHA一致；最终源码可由完整109与126 delta重建。原113同样保持原作业。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 SQLXML绑定验证候选局部终态：原132实际1PASS7FAIL行为RED保留；133在唯一标准SqlXml增加驱动验证、同一RdbcSqlXml独占锁串行委托，BufferedSqlOutput验证成功后才发布属性，默认未知绑定状态明确Unsupported，不提前读取、不消费读写机会、不隐式关闭调用者流。原133行为8/0后严格Clippy因夹具多余to_owned/无await helper失败；134仅修两夹具诊断，原八测试及断言和三生产源码不变。134最终绑定8/0、指定严格Clippy0、原SQL映射11/0、完整RDBC allfeatures 545/0、fmt0；1235完整冻结输入变化0。zip136 SHA 132b853e0c74b9b83bda9cde8df7c3bc63ca45a4e9b09abc6c9720cc0748bb01含原RED/质量失败/最终GREEN、固定Java SQLOutput原件及相对126的四路径delta，全部解压SHA及E镜像一致。Ruling: XML框架写状态由同一驱动验证，不通过读取XML猜测，不增加第二资源端口；代价是未知驱动必须实现验证后才允许绑定。Druid代理当前尚未覆盖该验证方法，后续须先执行owner RED再委托并验证fatal/nonfatal/关闭/free/取消。custom type map及最终全仓/API/实库未闭合，正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Druid SQLXML绑定委托阶段证据：135原六合同实际0/6行为RED，140新增父级关闭等待验证后取消交接，原七合同实际0/7 RED，均1235冻结无变化。137仅一生产代理方法增加等待payload锁前后生命周期检查、无admit的同一驱动委托及释放锁后原错误分类；七原断言不改。已完成binding7/0、完整owner112/0（原105保留）及三个指定目标严格Clippy0；原session79819控制Python61456与直属Cargo现场确认存活，正在pooled-stream编译，后继Druid lib与fmt未取得终态。135启动脚本第一次设置错误Cargo未执行，138保存现场并按134快照恢复后修正，未计产品RED。zip145 SHA b2e825b05744d52ad2a1c91586c82a1bd25d1665643ce4994fbad572b9eeefd0为136父链的两路径delta，保存全部真实RED与三个已完成GREEN、冻结清单及部分运行日志，逐项解压SHA及E镜像一致；部分日志不是通过证据。141终态归档脚本已准备，须137全部六命令实际exit0才可运行。正式Rust未apply；原113/129完整回归继续各自冻结快照，工具补齐确认仍待回复，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 custom type map合同启动：固定OpenJDK 7373384725418fa645fb4508ec05a869bc7fc3e7 SQLInput#readObject原件SHA逐项匹配已取得oracle。独立d146复制137冻结1235输入后增加五项真实合同，构造Cell非Sync应用SQLData、原限定UDT名/新实例、SQL NULL/无匹配Struct身份、不存在构造器拒绝、原构造错误及递归映射，合计1236冻结；未写生产实现。Ruling: 保留现有描述符RdbcTypeMap与canonical对象，通过实例级Rust构造器registry适配Java反射；同一SqlData只以mutex独占访问，不能用类名或JSON替代。原146/session31290已实际Cargo101终态、冻结变化0；诊断同时包含缺失标准映射入口与两处夹具错误，不能计纯产品RED。预检发现原夹具引用不存在的RdbcError::FeatureNotSupported及Cell借用跨await；修正版草稿仅改为现有kind分类并先await局部值再set，尚未应用运行中输入，原终态保留，须先排除夹具错误，不能记纯产品RED。zip147 SHA 12ef52bbf1239134f0a17f6bdaa85ef3bcd0ef8ea2c19af02ee5b37a4df18958保存原冻结测试/修正草稿/简报/Java原件/冻结清单及启动进程证据，全部解压与E镜像SHA核验；不是PASS。137池化/Drop实际9/0，后继Druid lib仍待终态；113/129完整回归保留原进程。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Druid SQLXML绑定委托局部终态：135首次runner选中summary内部commands=[]造成SyntaxError，Cargo未执行；138保存原runner/部分夹具并按134冻结恢复，修正独立命令列表定位后再执行，未计产品RED。135原六合同实际0/6行为RED，新增139父关闭等待验证后取消清理交接，140原七合同实际0/7 RED。137仅druid_sql_xml.rs覆盖唯一SqlXml验证，等待payload锁前后检查父级/登记，委托同一driver且不调用admit，释放锁后保留原结果分类；七原断言不改。最终binding7/0、完整owner112/0（保留原105）、池化/Drop 9/0、Druid lib 153/0，指定三目标严格Clippy0、fmt0；1235完整输入变化0。zip141 SHA f93536322fb7c1508f9b509115341ee67f71ff795a21ef7b2cebd02475938528含原设置失败、实际RED/GREEN、两路径delta及69份Java来源SHA重检证明，全部解压与E镜像逐项核验，可由136父链重建。Ruling: 绑定验证属于同一标准资源操作，不消耗读写机会、不绕过父级、不隐式关闭调用者资源；取消交还锁后由原监管队列完成清理。未知驱动默认拒绝仍需实库/provider验证，custom type map、RowSet与最终API/SemVer/MSRV/compiler/全仓仍开放，正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 custom type map候选局部终态：原146编译101同时包含缺失入口与两处夹具错误，不计纯产品RED；148只将不存在异常枚举改为原kind分类、Cell.set(await)改先await局部值，保留Cell非Sync及全部五合同，实际干净E0432/E0599标准入口RED，1236冻结变化0。149新增实例级SqlDataRegistry和SqlDataObject两个一对象一文件生产类型，BufferedSqlInput显式安装原type map/工厂，匹配限定UDT后构造真实Send-only SQLData、按原声明顺序readSQL，嵌套继承同一目录；映射失败不发布半成品，原错误传播，NULL及无匹配原Struct身份保留。现有RdbcTypeMap/RdbcObject与driver registry不改。官方定向rustfmt保留五原合同，最终映射5/0、指定严格Clippy0、完整RDBC allfeatures 550/0、fmt0，1238完整冻结变化0。zip150 SHA ce995c8c640fc44d37867734103a5174aac17a0dedc7768bf1199329eb8bd3bf保存原混合失败、干净RED、最终GREEN、固定Java原件/简报、完整清单及相对141终态父链的五路径delta，全部解压与E镜像核验。Ruling: 用显式构造器适配Java反射，用对象级mutex保留可变应用实例及Custom身份；不要求SQLData Sync、不通过类名/JSON冒充对象。readSQL错误/取消、各provider type-map接入、DISTINCT实库、RowSet及最终API/SemVer/MSRV/compiler/全仓仍需完成；不宣称完整JDBC语义或驱动认证。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 RowSetMetaData标准候选阶段证据：固定OpenJDK7373384725418fa645fb4508ec05a869bc7fc3e7 javax.sql.RowSetMetaData原件与69来源oracle SHA匹配，完整17个setter身份留存。原152生产不改仅增五合同，实际E0432公开面RED，1239冻结变化0。153新增唯一RowSetMetaData必需方法trait、真实RowSetMetaDataBuffer稀疏实现及私有一列状态；同一ResultSetMetaData标准getter/owned快照/Wrapper leaf-parent-concrete-mutable身份闭包，原五合同含完整17setter、非法值不变、nullable名称、列隔离/重置及i32MAX惰性状态，实际5/0、指定严格Clippy0，1242冻结变化0。完整RDBC原session50786控制Python61956及直属Cargo运行状态已现场观察，未取得本检查点完整PASS；初次编译增量缓存AccessDenied note保留原日志，不据此重启。原68入口探针154已准备但尚未执行，不推定62/6或认证RowSetMetaData精确kind。Ruling: 这是标准接口与Rust缓冲实现，默认flags false/nullable Unknown/空名称遵循接口；不冒充另一个Java RowSetMetaDataImpl具体类（其未初始化nullable String及MAX特殊分配行为未认证）。追加固定源码仅供setter校验参考，不改69原oracle分母。zip155 SHA b05de65e195383bcdb4420134b96021ebad116773449dc31f31fb3d173081d9e为150终态父链的八路径delta，保存原RED、两已完成GREEN、完整冻结清单、Java原件/17方法/简报及明确标记的部分回归日志，逐项解压及E镜像SHA核验。其余六RowSet入口、provider type-map接入、完整API/SemVer/MSRV/compiler/全仓及18顶层任务仍开放，正式Rust未apply，Task6 FIX_REQUIRED/Task8 INCOMPLETE。


2026-10-09 RowSetMetaData候选局部终态：153原四命令全部实际exit0，五项metadata合同5/0、指定严格Clippy0、完整RDBC allfeatures555/0、fmt0，1242冻结变化0。154以新Cargo JSON唯一rlib执行原完整68公开入口探针，原身份/形态声明/目标/范围/源码SHA逐项不变，实际62PASS6FAIL，另1host未认证；较125的61/7仅改善RowSetMetaData，剩六RowSet项保留。入口解析不等于精确kind、Java行为或SemVer认证。原152 E0432 RED、17setter来源、全部153/154日志及清单、相对150父链八路径delta归档zip156 SHA 09a675fe891c8dbd87688e452d49bed66e393d11fca1ac9c8b410fd6dac0b181，逐项解压与E镜像核验；155仅阶段快照保留。补充Java RowSetMetaDataImpl仅参考验证，不声称具体类完整对等、不改69oracle。正式Rust未apply；provider接入、其余RowSet及完整API/SemVer/MSRV/compiler/全仓仍开放，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 最终语义CLI候选终态：独立d157逐项复制153冻结1242输入，新增最终入口合同和运行证据组件，仅修改原semantic verifier，共1244输入。原CLI --require-final-evidence真实exit2/unknown argument及调用unexpected keyword RED保留；新增CLI接通已存在最终来源重建与状态策略，禁止跳过adapter/logical evidence，保留显式Task3 canonical要求。运行组件要求当前cargo-public-api形态（非原68import探针替代）、final purity、原完整no-Physical含C1/C2、Wrapper compiler/runtime及Druid零Task6/7 pending；工具缺失/编译失败/未闭合资源不能返回成功，原执行器超时和分母不变，未安装工具。新增协议/API输入冻结缺失实际RED后补完整来源冻结；来源加载期oracle替换实际23项中1FAIL RED后将oracle快照提前到load之前，原错误未隐藏。159最终原35自测与新23合同实际全通过，1244冻结变化0；新合同中的合成API文本/组件mock只证明解析和委托/失败关闭，不冒充真实compiler/runtime认证。实际CLI当前输入仍exit1：row2 PLANNED_TARGET，未运行下游门禁，最终验收仍未通过。zip160 SHA ab5670b5fab1f8e6f5180bb9049d698193a366a322be13a05b030e27363bb9e9保存原RED/中间已验证源码/159日志与三路径delta，全部解压及E镜像SHA核验，可由156父链恢复。Ruling: 最终semantic gate拒绝计划态与来源漂移，再要求新鲜编译和原运行证据；API/SemVer/实库/性能及全仓仍按Task8/P07独立验收，不把CLI测试通过算产品完成。正式Rust与新CLI均未apply到E；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 SQLData映射失败与取消合同终态：独立d161从159逐项复制1244冻结输入，仅增一个真实合同文件，所有生产/原测试/工具源码字节不变，合计1245。固定Java SQLInput/SQLData原件SHA匹配既有69oracle；新增四合同通过标准SQLOutput生成真实Struct、同一显式type map/应用factory进入readSQL。实际单层/嵌套readSQL错误完整保留vendor/SQLState/null消息/运行类/cause与next链，未发布实例各drop一次；先真实poll进入readSQL及Pending再取消，单层/嵌套实例各drop一次，后续99属性仍可读取。四合同首次实际4/0，既有实现正确，无产品RED或生产修复声明；严格Clippy0、原五项映射5/0、fmt0，1245冻结变化0。最初runner在复制未完成时缺parent-inputs文件，Cargo未执行，保存设置失败事实；原clone完成后同一runner正常执行，不算产品RED。zip162 SHA ac5122cb78b768bedded5928c29b0e87a52c3d2af703809dc21319c680bd867a为160父链的一路径测试delta，含实际命令/日志/来源原件和完整SHA清单，全部解压及E镜像一致。Ruling: 取消合同只证明未发布映射实例的Rust所有权清理，不扩张为外部数据库事务回滚或任意驱动资源释放认证。此局部readSQL错误/取消缺口已取得证据；provider type-map接入、DISTINCT实库、六RowSet入口、完整API/SemVer/MSRV/compiler/全仓仍未闭合。正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。

2026-10-09 RowSet参数状态候选局部终态：原163冻结1246实际E0432标准入口RED；164新增真实RowSetParameterStore与私有RowSetBindingKey，复用完整PreparedInputParameter，保存32描述符族及NULL/Calendar/长度重载/原流身份。名称仅由provider解析，同一实际位置最后setter生效，重复selector不积累历史，极端稀疏index在分配前拒绝；get/snapshot不消费，clear不隐式close调用者资源。原六合同保留，最终6/0、指定严格Clippy0、完整RDBC allfeatures565/0、fmt0，1248冻结变化0。zip165 SHA f92741de75ba53e79c2b228631104b5f8142ac8b9cb70136d9ed4e7aae7ee9d7保存干净RED、最终完整日志/命令、两固定Java原件/115+5方法清单、简报及相对162父链四路径delta，全部解压SHA与E镜像核验。原runner继承SQLData ruling与original_five字段属于历史元数据，本归档明确六RowSet合同及163简报为本阶段解释依据。Ruling: 参数组件是完整RowSet的组成部分，不替代RowSet；execute/events/source identity/original rows/provider writeback及六公开入口仍开放。正式Rust未apply；原113/129全仓作业现场确认原Python仍存活，不改变输入或重启。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。

2026-10-09 RowSet属性候选与完整RDBC失败终态：166干净E0432 RED后，167新增单对象RowSetProperties及mod，26固定Java属性声明逐项映射，原六合同仅官方fmt且源码核验。实际6/0、指定严格Clippy0；完整RDBC实际570PASS1FAIL/Cargo101，唯一ConnectionCapabilities doctest编译缺libdruid_rdbc-74ae33e7e3ce0c83.rlib，不是断言失败；fmt后继未执行，1250冻结输入变化0。原167与169/170不同源根共享target交叠，失败后同名rlib于06:46:07重新存在；缓存覆盖为有事实支持的推断，具体删除动作未追踪，不夸大认证根因。zip173 SHA 3c94cf55601c1cfd0140e4abd67e7abed7b4f69059c592caaa4509b49d68bbe3保留真实RED/6GREEN/严格GREEN/完整失败日志、Java原件/26映射/简报/原未执行168脚本及相对165父链三路径delta，全部解压与E镜像SHA核验。172已用原d166/1250原字节、全新独立target及external-target运行同一完整RDBC与fmt，尚无终态；不改原断言/测试分母/超时，不把后续成功倒写167。Ruling: 此后不同候选源根必须隔离编译与external-consumer缓存；源码冻结不能替代产物隔离。完整RowSet执行/事件/原始行/provider写回及最终API/MSRV/实库仍开放；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。

2026-10-09 RowSet原始行与更新状态局部进展174：169从167冻结1250逐项复制（clone时父完整回归尚运行，后来167真实570/1保留），新增六合同干净E0432缺RowSetRowStore/RowSetRowChange RED。170新增三一对象一文件实现：私有单行、公开变化枚举与rich-row存储；保留原始/当前/pending分离、Reader引用与UTF16游标、原始行无消费、宽度/索引原子拒绝、取消/提交更新、新增无原始值、删除原行保留冲突值、新增后删除无数据库变化、只有显式成功accept才重建原基线。原六合同6/0、指定严格Clippy0，1254冻结变化0；完整RDBC原170仍未认证。只读官方rustfmt预检exit1，仅三mod声明排序，source未修改，修正草稿留存，必须等原170终态后再应用并验证。机器174记录原RED/两已完成命令SHA、五路径delta及格式预检，并镜像E；不是完整终态归档或PASS。Ruling: 行数据不是标准ResultSet，getOriginal/getOriginalRow仍须before-first适配；writer false/Err/取消不得提前accept，此组件不执行provider或承诺数据库回滚。六标准入口/事件重入/Reader通知抑制/execute/provider实库及最终门禁仍开放；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。

2026-10-09 RowSet属性候选独立缓存终态：原167完整570/1/Cargo101及缺rlib诊断保留在173失败终态归档，不删除或改写。172使用原d166同一1250冻结源码/测试/工具原字节，新建独立CARGO_TARGET_DIR及DRUID_RDBC_EXTERNAL_TARGET_ROOT，完整同命令RDBC allfeatures/locked/offline/-j1实际571PASS0FAIL（含此前doctest）、fmt0，终态输入变化0。结合原六属性合同6/0和指定严格Clippy0，取得此属性候选局部完整证据；无业务源码修复或放宽断言/超时声明。zip176 SHA d8c0c3a3c3ddb37fc7772e49dba26c073cc054f1cb264e7738f4645388183b77为173父链零源码delta验证补充，保存172完整命令/日志/清单、缓存隔离裁定及原167失败，全部解压与E镜像SHA核验。Ruling: 不同候选源根必须隔离编译及external-consumer缓存；本次独立缓存成功支持隔离措施有效，不冒充逐系统调用删除根因认证。170原始行候选及完整RowSet六入口/执行/事件/provider写回、最终全仓/API/MSRV/实库仍开放；正式Rust未apply，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 完整RowSet行为合同RED181：原177从格式排序后的169快照逐项复制1254输入，新增11项真实执行合同；独立Cargo/external缓存实际Cargo101，仅E0432缺失标准入口，1255冻结变化0。原177终态后179保留原11合同字节前缀，再增外部clone原子可见性、Writer真实poll后取消、getOriginalRow游标及删除原始行三合同，共14项，实际再次干净E0432 RED，生产源码未修改。固定RowSet115/RowSetInternal5声明原件SHA复核并留存完整义务清单，未将入口或14合同代替全接口/Java行为认证。Ruling: Reader在执行上下文暂存，外部clone在await期间只见已发布基线；成功原子发布后释放所有锁再通知同一source事件，错误/timeout/drop只丢弃暂存，不声称数据库回滚。原171仅因官方LF与草稿CRLF字节比较失败，Cargo未执行，保存设置失败和实际格式化结果；178同六原合同6/0、严格Clippy0，完整RDBC/fmt尚未认证，不将原170的577/0扩张到修正后快照。zip181 SHA 1900ef031b4fbf609d4750b4a3e4df3b2d618018e9b7f138ce41b0edd88e4c81为176父链六路径候选delta，保留原11与14合同RED、完整1255清单、六Java原件、实施简报与设计及171失败事实，全部解压SHA和E镜像核验。六标准对象及具体RowSet实现仍待完成，正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 RowSet rich-row状态格式修正后完整终态184：原170六合同6/0、严格Clippy0、完整RDBC577/0、fmt1均保留。原171仅预览CRLF与官方LF字节比较失败，Cargo未启动；规范化后完全相同，唯一源码差异为三mod声明顺序。178在原作业终态后使用同一源根重新验证：原六合同6/0、指定严格Clippy0、完整RDBC577PASS0FAIL、fmt0，1254输入终态变化0。没有修改原断言或方法体，不把设置失败当产品RED，也不删除原fmt失败。zip184 SHA cfda3be66021e10e282d390d9be650bcc07562ad2126c6f6ae07d4688819a844为176父链五路径delta，保存原干净RED/原失败/修正后全部命令与日志及冻结清单，全部解压SHA与E镜像核验。Ruling: 这是rich-row存储局部验收，不替代完整RowSet/ResultSet、Reader/Writer、事件、provider写回或数据库事务证明。完整14合同已在177/179取得缺标准入口RED，182完整接口及执行实现正在F候选实施，尚无通过结论。正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 RowSet核心候选191：186原14行为合同14PASS/0FAIL、严格Clippy exit0；1268冻结输入变化0。包含完整115声明接口、Reader/Writer原子发布与取消、事件重入、共享metadata和调用者连接借用。保留182编译失败、185严格检查失败及修正前源码；测试异常构造修正为显式装箱，原14测试方法体断言保持不变。zip191 SHA a75864f7c6aac9f1d467eb1b36d9eb9c6b73e5cdd96437ad181cb6e89a6cedf8，父184，原干净RED181作为关联前置证据，19源码delta路径。另188全表面测试85重载与属性PASS，Wrapper胖指针ptr::eq判定失败，正在189以addr_eq检查对象数据身份并继续全量回归，不将测试判定失败当生产缺陷。尚无新全量RDBC通过结论；外部RowSet Wrapper宏继承闭包待补验。正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 RowSet外部Wrapper宏RED/预验197：实际d177编译产物上的外部消费者调用impl_standard_wrapper!(ExternalRowSet => RowSet)，rustc exit1，唯一缺RowSet宏匹配规则，源/rlib/proc-macro输入SHA保持不变。193准备普通及generics-where宏臂；195将精确候选宏在外部消费者中独立展开，普通/泛型const+where身份2PASS/0FAIL。此为preview，未重建canonical库，未apply生产或d177冻结源码。194拼接内doc位置错误原日志保留，195只修预验拼接位置。196实施runner已准备，强制189成功终态及1269源输入一致后才允许写入，当前未运行。189已取得85setter/属性/Wrapper三个表面合同3/0、原14行为14/0、两目标严格Clippy0；完整RDBC仍运行，不声明全量通过。zip197 SHA 86deb250f6b72d6260abebbc0fe3c5714967feed2d2618c28d3d10e47214da2d。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 RowSet实际Pending游标准入RED201：198外部消费者编译exit0；Reader/Writer真实poll进入Pending之后，外部clone.move_to_current_row均错误返回Ok，两个行为测试0PASS/2FAIL；冻结source/rlib/proc-macro/tokio输入变化0。199准备owner锁内状态检查与cursor更新，避免检查后执行开始的窗口，并将insert资源引用析构移到锁外；仅草案未apply。200合并Wrapper宏修正与两个新测试，前置189成功终态与全源哈希门禁，尚未执行。zip201 SHA ed96f7013c11bafd776c1b94e1180586252bc667b5007e33cf89380801aa3040，8条目解压与E镜像SHA核验。189全RDBC仍执行Wrapper外部编译测试，不以局部17PASS/strict0替代全量或新缺陷修复证明。正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 完整RowSet候选表面与全量RDBC终态202：189表面3/0（85 setter重载、26属性、Wrapper身份）、原行为14/0、两目标严格Clippy0、完整RDBC594PASS/0FAIL、fmt0；1269冻结输入变化0，日志SHA核验及全部zip条目/E镜像SHA核验。zip202 SHA 5b4b118e7ca4ca493eda7db2357a799c6173cf8c2b6e5d1099bff769ce50062c，父191单测试路径delta，保留188胖指针vtable比较失败与修正后的数据地址身份判定。该全量通过不掩盖新增192外部宏缺规则RED和198Reader/Writer Pending游标准入双RED；200将在此成功终态之后应用两生产文件与两测试文件，必须重新验收。正式Rust未apply，真实驱动/完整公共API/MSRV/发布门禁仍待完成；18顶层OPEN。


2026-10-09 RowSet两处真实缺陷修正定向验收206：在189成功终态及1269全源SHA核验后，200应用Wrapper宏（普通/泛型where闭包）及Pending外部游标准入检查+更新原子临界区；实际外部消费者宏2PASS/0FAIL、Reader/Writer实际Pending双合同2PASS/0FAIL。200严格检查exit101仅新夹具四处void委托缺分号，原源码/日志保留。204仅四分号修正，测试断言与生产实现不变，原两组各2/0、四组目标严格Clippy0，1271冻结输入变化0。zip206 SHA 94ccf0ec374e432de0d36daae2c5e7493e3f4492d7d911f0164d84c3e54cda65，父202四路径delta及原197/201 RED引用，22条目及E镜像SHA核验。204新完整RDBC尚在运行，205原68公共探针runner已准备待成功终态后使用Cargo实际选择的产物，未运行不宣称通过。正式Rust未apply；18顶层OPEN。


2026-10-09 RowSet前向游标能力RED209：固定Java ResultSet 1123/1138/1241/1266/1317行明确TYPE_FORWARD_ONLY禁止scroll和非FORWARD fetchDirection。207实际候选库外部测试编译0，前向导航/反向fetch两个行为RED、ScrollInsensitive正向对照PASS（1/2），输入源/rlib/tokio/Java SHA变化0。208准备状态与能力owner临界区校验；原第14测试after_last遗漏可滚动请求，仅补set_type(1004)前置条件，全部原断言保持。210runner以204成功终态、205原68公共探针终态和全源SHA一致为前置门禁，当前未运行。zip209 SHA 514ebae67bb04a0226e00700bb1b0c89a32ff72270b4715b065e08d1c3361552，12条目及E镜像SHA核验。204 full RDBC仍运行，205未执行，禁止以旧594全量或局部4PASS替代新能力修复证明。正式Rust未apply；18顶层OPEN。


2026-10-09 RowSet Wrapper宏/游标准入修正完整终态211：204外部宏2/0、真实Pending Reader/Writer准入2/0、四目标严格Clippy0、完整RDBC598PASS/0FAIL、fmt0；1271冻结源输入变化0，各完整日志SHA核验。zip211 SHA f7f3b9ecd69afaa716937c82ba2103f9c7e99d835d79d6eda8c6a4d6bbcce44a 为206零源码delta终态补证，全部条目/E镜像SHA核验。原200夹具分号检查失败及修正保留在206，不隐藏失败。新207前向能力双RED仍待210修正；不得用598通过掩盖尚未覆盖的能力错误。205原68公共接口探针即将执行，尚无结果。正式Rust未apply；18顶层OPEN。


2026-10-09 原68公共类型入口探针212：204成功终态之后，205以Cargo build --message-format=json实际选取的rlib重编译原68未改变探针，68PASS/0FAIL；此前154的RowSet/Event/Internal/Listener/Reader/Writer六缺失入口已补齐。1271冻结输入和rlib SHA不变，原probe SHA逐条核验。zip212 SHA 6810fd2862b189e30b19ef22ab8ae6a8ec9ccd30f780589b8c53b872686bf016，父211零源码delta，全部条目及E镜像SHA核验。严格限于import/trait-bound，69语义行尚有1宿主非Rust项；未证明完整enum/module形状、Java行为、SemVer、MSRV，Task8仍INCOMPLETE。210前向能力修正现在具备前置门禁，尚未通过前向修正验证。正式Rust未apply；18顶层OPEN。


2026-10-09 RowSet前向能力修正完整终态214：210在204及205成功终态后应用前向能力校验，新增3合同3/0，原14合同14/0，五组目标严格Clippy0，完整RDBC601PASS/0FAIL、fmt0；1272冻结输入变化0。原第14 after_last 测试仅补set_type(1004)正确前置条件，源码校验移除此唯一新增行后全部原测试方法体字节相同，未修改断言。zip214 SHA cc36e37099c6978789620269afc942385f314a6980e59f45d422b1daab329133 为212父链三路径delta，关联209固定Java及真实双RED，全部条目/E镜像SHA核验。213将以新实际产物再运行原68公共探针，尚无该次结果。正式Rust未apply；完整workspace/C1/C2/Java行为/API形状/MSRV/发布门禁尚待收敛；18顶层OPEN。


2026-10-09 前向修正后原68公共探针终态216：210完整RDBC601/0、五目标strict0、fmt0之后，213用Cargo实际选择的新rlib重编译原68未改变探针，68PASS/0FAIL；1272冻结源输入与产物SHA保持不变。zip216 SHA 8ae7046eb73948ff9f93bea700e356bc091e563b25ef2d6ed3fcf93f2242d4c4，父214零源码delta，全部条目/E镜像SHA核验。仅证明公共import/trait-bound；1宿主项、完整API形状、Java行为、SemVer、MSRV仍未认证。215已准备精确1272源码独立克隆及独立主/外部缓存，执行workspace全量、严格检查、doc、fmt与正式C1/C2；不让旧113/129快照替代新源证明。正式Rust未apply；18顶层OPEN。


2026-10-09 新完整收敛作业215启动217：210成功终态601/0+strict0+fmt0、213原68探针68/0之后，精确1272输入从d177复制到d215并逐条SHA核验，source_copy_verified=true。实际会话96326已启动，Python PID 12388，running_command=workspace-complete；主cache task7_full215_target，外部cache task7_full215_external，与仍在运行的113/129及d177独立。依次执行workspace all-features no-fail-fast、all-targets strict、doc、fmt、正式C1（含C2），保留原超时门禁，不降格或删除失败。当前仅STARTED_NOT_PASSED，禁止将旧快照全量或RDBC601/0替代该新完整工作区证明。开始快照已E镜像SHA核验；正式Rust未apply；18顶层OPEN。


2026-10-09 最终API单元枚举成员识别222：218原7合同5PASS/2ERROR，缺少实际cargo-public-api格式的单元枚举成员识别。候选仅修验证器并增加原7合同，必须完整成员路径及直接父枚举声明，不推断缺失成员，不接受常量/字段/DTO冒充；原enum/struct/module/trait检查保持严格。219新7、原CLI23、原语义35、原公共入口10共75自测实际PASS，套件名称与数量以本记录为准。219实际CLI使用不支持的--repository-root而exit2，原失败脚本与日志保留；222在原cwd用真实支持参数重新执行，exit1/row2 PLANNED_TARGET，下游未执行，不晋升清单状态。1273冻结输入终态变化0，相对210仅两个Python路径delta，Rust/原测试/库存清单未改，d215仍保留1272原快照。zip222 SHA d130e3beb6b4edfc898957c0ea24817951fe612ca08367422d3d689aa03fb9ed，父216，全部条目/E镜像SHA核验。历史API只作为格式夹具，未执行新cargo-public-api或认证真实API/SemVer。正式Rust与CLI未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 Task8当前源码形状审计221：固定现有69行清单，对照F候选直接声明及具体来源行/SHA，确认BatchUpdateException、DataTruncation、SqlClientInfoException、SqlException、SqlWarning五项ENUM_KIND对应struct，DriverRegistry的MODULE也对应struct，共六处须按已接受合同解决。该报告是源码/映射冲突定位，不是新cargo-public-api认证；未修改清单或放宽形状验证。220误用仅含枚举片段的格式夹具遍历全清单，其余缺失不构成当前API失败，已在221明确排除。221 JSON SHA 5cde516bf68a1738eab00140adfaf42cde741b2a44bb7bc71f9f0a149b2bb18e，E镜像核验一致。222原75测试GREEN及实际最终CLI row2 PLANNED_TARGET/exit1保持；正式源码未apply，18顶层OPEN。


2026-10-09 SQLx真实异步前向游标RED226：224在1274冻结候选上实际cargo test/default/locked/offline编译成功，三合同2PASS/1FAIL，仅ForwardOnly set_fetch_direction(Unknown)错误返回Ok；Reverse拒绝与ScrollInsensitive接受Unknown均PASS。原生产/原测试/工具未改，终态输入变化0。固定Java ResultSet#setFetchDirection原件SHA 42718dec5d2fe53fd994c5cb9b5521729e8070f5f6461064d79908a521d3c2d6明确前向游标禁止所有非Forward值。225最小修正只接受Forward，新测试仅官方rustfmt换行，不改断言；原3合同、原async目标、两组strict、fmt及完整Wrapper默认功能回归正在顺序执行，尚无GREEN终态。zip226 SHA 98c71c5c50fccc16cf52f9e324fc8a8281ad77f445087f3c927d99e91afc69c4，父222单新测试delta，保留干净RED日志、源码/测试原件、Java来源与完整冻结清单，全部条目/E镜像核验。不同源根缓存保持隔离，原215全仓1272快照未改；正式Rust未apply，18顶层OPEN。


2026-10-09 共享标准资源释放重试RED230：227外部夹具缺Future import编译失败，原源/日志保留，不计产品RED。228仅补该导入，同一213实际选定RDBC rlib且全部当前RDBC源码SHA一致，rustc编译0；三行为2PASS/1FAIL。首次已知free错误后可读与取消后成功重试/重复free无副作用通过；取消进入ReleaseUnconfirmed后重试Err错误恢复Open，length进入驱动，失败断言明确要求未确认资源不得重新开放。源码/rlib/宏/futures输入终态变化0，未修改运行中225源码。229准备锁内保存先前状态，重试Err仍保留3，仅首次已知Err恢复0，成功确认2；草案未apply，必须等225成功终态及全源SHA门禁后实施验证。zip230 SHA 22d05bf741dde1097fb210cefdaf24c85f118759c53f0d7bd4a0add8c6858e77，13条目及E镜像逐条核验，关联226证据链。225前向方向原3合同已3/0，原async/strict/fmt/完整Wrapper默认回归仍运行；215全仓仍独立1272冻结。正式Rust未apply；18顶层OPEN。


2026-10-09 资源重试后继受控启动233：231/session40922原Python 62012现场确认存活，当前candidate_applied=false，等待225成功完整终态。只有225五命令全通过、232终态归档及1274全输入SHA一致后，才允许应用229标准值最小修正与228原三合同，重新冻结1275输入；随后新三合同、资源strict、全RDBC、Druid owner取消、fmt顺序运行。225新前向合同3/0及原15异步合同15/0日志已取得，strict/fmt/完整Wrapper默认仍未认证；原215全仓也仍独立运行。233机器启动快照与两执行脚本SHA已E镜像核验，STARTED_NOT_PASSED，不以启动代替通过。正式Rust未apply；18顶层OPEN。


2026-10-09 释放取消后排队重试合同RED237：234新增两合同，错误期待读取在state1排队而产生一项夹具失败，原5测试2/3日志保留；235只修该控制为立即拒绝state1及取消后拒绝state3，不二次poll已完成future，原直接/排队重试断言未变。rustc实际编译0，五合同3PASS/2FAIL；两失败均重试Err后错误恢复Open，包含重试先排队后获得锁的路径，证实状态须在锁内取得。原三个对照为初始已知失败可读、成功重试恰好一次、释放中/取消后读取拒绝。来源/rlib/宏/futures输入变化0，未改F候选或运行中225/231。zip237 SHA b6730712879b26d37e71a8daad3b2b82b6ba9c7e95e720cf3a759abf5b728d6a，父230证据链，全部条目/E镜像SHA核验；229锁内保存前态草案仍未apply。后续须用修正后新鲜编译产物重跑完整原五合同，不以原三合同替代新增排队路径。正式Rust未apply；18顶层OPEN。


2026-10-09 集成保护只读快照236：225冻结1274中1254代码/支持文件与E逐项对比，1090相同、68不同、96在E缺失；E HEAD f50787031ce64fbc85ebdbe83ab8670a4da05b7b。记录当前E preimage与候选SHA，报告SHA 3347222bd7f5a6b9f87d3df9cbeccc679749bea8d75441337d7037dea03e1b01，E镜像核验一致。差异不是覆盖授权、最终集成清单或完成认证；最终候选确定后须重算desired SHA，逐项保护E用户改动，231后继资源修正不包含在该快照。正式业务源码未修改；18顶层OPEN。


2026-10-09 原五资源合同后继受控启动241：238/session87850 Python 31440现场存活，candidate_applied=false，等待231完整成功终态。先保存231原三合同/全RDBC/Druid取消/fmt终态239并核验全部1275源SHA，再仅将测试扩展为原235五合同；原三方法及夹具前缀文本完全相同，生产源码不变。执行新五合同、严格Clippy、完整RDBC、fmt，全部须实际exit0；新增排队路径不得用231三合同替代。启动快照E镜像SHA fc940d1ce84cdf8ea42025f3667d09af82ef0c28db130b35c1bc1c8e381c5618；STARTED_NOT_PASSED，不修改运行中225/231候选；18顶层OPEN。


2026-10-09 三方集成审阅242/243：基于236精确E preimage和225候选，对164差异路径生成完整review diff；96候选新增、1个E等于HEAD、12个E已有未跟踪路径、55个双方均不同于HEAD。55个git merge-file原字节三方预览中12文本无冲突、43文本冲突；保存全部current/base/candidate/merge-output/stderr，输入前后哈希一致。文本合并状态不是语义正确性、所有权或覆盖授权；HEAD不是所有候选变更的实际共同祖先，43冲突须结合迁移来源和原合同逐项处理，不能机械用ours/theirs。zip243 SHA 890390c95734bc4c6e45090c69718dbe4a7711ab1da503005c9ab1fe686edb8c，440条目/E镜像SHA全部核验；231/238后继差异未包含，最终候选后重算。未覆盖E源码，不把review artifact计为已集成；18顶层OPEN。


2026-10-09 集成审阅消除文本无冲突假象244/246：243的12个clean输出中11个原字节等于候选；actor另一个clean输出重复插入execute_query_with_generated_key与execute_update_keys。用现有Rust词法器逐token核验完整fn签名/体，E与候选两个方法各一份且完全相同，raw merge各两份亦相同，故review版本保留候选各一份，未丢E生成键逻辑。12个resolved review原件已准备，仍需最终编译/合同验证，不能当交付。245补查candidate方向对比遗漏的11个E-only路径：10个旧resource access/context/kind/state模块及旧SqlxStreamingResultSet；全部SHA冻结，必须明确保留/退休处理，未删除。zip246 SHA 143a9a5c6cd73ab739a69c99e93f8214249a7d7af28783b0ad1c875ebea44fd7，14条目/E镜像SHA核验；43文本冲突与12已有未跟踪路径仍需逐项处理，231/238最终后继未含。正式源码未apply；18顶层OPEN。


2026-10-09 SQLx前向游标修正完整默认Wrapper终态232：225新三合同3/0、原异步合同15/0、两目标严格Clippy0、fmt0、完整Wrapper默认功能363/0，1274冻结输入终态变化0；原224真实2/1 RED保留。zip232 SHA 7eeabc7675423f17d7228509281425bbe4ba34c847fde684b1c2faaf1e80c5d5，父226两路径delta，全部条目/E镜像SHA核验。此范围为Wrapper默认功能，非allfeatures/实库全矩阵或全仓通过。共享标准资源取消后重试错误另有228真实2/1 RED，不能由此通过掩盖；231将在此成功终态归档及全源SHA门禁之后实施最小修正并重新验证。正式Rust未apply；18顶层OPEN。


2026-10-09 资源修正已在候选实施：225完整默认Wrapper363/0终态与232归档/E镜像已核验，zip232 SHA 7eeabc7675423f17d7228509281425bbe4ba34c847fde684b1c2faaf1e80c5d5。231通过前置1274全源SHA门禁，实际应用standard_resource_value锁内前态保留修正和原228三合同，冻结1275输入；两个delta SHA现场一致，当前retry-green编译执行中，未取得通过终态。238原五合同仍等待231完整五命令通过，不以当前启动代替排队重试验证。正式E业务源码未apply；18顶层OPEN。


2026-10-09 E-only旧入口处置247/248：逐项冻结11个E-only文件原件，按已批准standard resource合并与SQLx buffered rename说明候选替代源、SHA和删除前完整C1/C2/API/resource/Wrapper/purity门禁。候选三个生产crate根的11旧对象名rg扫描exit1/无输出，1275冻结输入前后核验一致；只证明源码引用消失，不是删除或行为验收。zip248 SHA 8e2dd39219d7500cc4588cdfbbd9a89b96b0a91900a17b1f61d282dfa3a035de，12条目/E镜像核验；未删除E文件。231原三合同已3/0，strict/全RDBC/Druid取消/fmt仍待终态，238五合同尚未运行；18顶层OPEN。


2026-10-09 配置实库后继253受控启动255/256：Docker服务只读预检成功，本地postgres18.6和mysql8.4镜像按image SHA固定，无下载/安装。253/session95786 Python 65152现场存活，等待238原五合同及full/strict/fmt成功与240归档SHA核验；未建容器/未运行Cargo。后继只新建本任务label且127.0.0.1随机端口容器，隔离继承DB变量，required=1运行原完整产品合同，显式配置bad credentials、长查询timeout/cancel及仅自有容器restart；finally核验label后清理自有容器/匿名卷。原249就绪夹具使用本地socket，启动任何容器/Cargo前停止并保留原脚本/终态，253/254改TCP probe并补240发布等待，不计产品RED。zip256 SHA 8c98c12b3721fe54bd0a580c7cca88b570b6207ab1dd04182dacb1ef5c558367，7条目/E镜像SHA核验，STARTED_NOT_PASSED；未生成产品认证/未修改支持状态/正式Rust未apply；18顶层OPEN。


2026-10-09 修正后完整候选验收258受控启动259：session5299/Python 48920现场存活，只等待253两个实库合同各1/0、完整日志/1275源SHA及全部自有容器清理后，才在d177同一主/external缓存顺序执行全workspace allfeatures nofailfast、alltargets strict、doc、fmt和原完整C1/C2；不与其他d177 Cargo并行，不缩减原超时/身份分母。215旧1272快照独立继续，其结果不能替代新增225/231/238生产/测试后的验收。启动快照E镜像SHA b451f7c8d0489b0e494b70257df8bded3cb71e80b1425875271a811aa560afa7，STARTED_NOT_PASSED；当前231三合同3/0和strict0，完整RDBC仍LIVE；18顶层OPEN。


2026-10-09 当前1275候选C2源码绑定预检257/260：原CLI --no-execute实际exit0，67文件/1283原测试身份，1146EXACT/137参数化/0纯身份/0批准移除，executed=no；冻结输入变化0，日志SHA cde5563baecc882f2ab4c633e52051fcba0a1ce7c857f61a9c6dbd3d353e5306。仅验证BASE/ledger/当前源码/观察闭包绑定，不声称已执行原测试、完整C2或关闭Task6。zip260 SHA ccc90424d6948c07be090017ae2fbf6492b5a851e9195853e9b5831a23c6511c，四条目/E镜像SHA核验，保留原验证器、完整日志、结果和231冻结清单。完整原C1/C2仍由215旧快照及258修正后候选各自实际执行，不混用快照结论；18顶层OPEN。


2026-10-09 资源修正231完整RDBC阶段结果补证：本轮重新读取原summary并计算完整rdbc-full日志SHA，实际exit0/604PASS/0FAIL，SHA 365d5f12371835486f936494d1c7df1c43314f78b0ef49b150808955bde4161a 与记录一致，changed_inputs为空；原三合同3/0与指定严格Clippy0已完成。现场CIM确认Python62012及直属Cargo68096存活，执行原Druid resource_owner_cancellation_test；session40922同一handle45秒观察仍LIVE，不重启、不改变运行中输入。231尚未全部五命令终态，238五合同、253实库与258完整验收仍等待前置，不把阶段PASS声明为Task6/7关闭或全仓通过。正式E业务源码未apply；18顶层OPEN。


2026-10-09 资源失败重试修正原三合同完整终态239：231五命令全部exit0，原三合同3/0、资源strict0、完整RDBC604/0、Druid owner取消112/0、fmt0；1275冻结输入变化0。zip239 SHA eac0038b2cf85499521c543f55c1c2f9f5cd18e7f2bf06ed7e76ccd7371a788d，父232，全部条目/E镜像SHA核验。原235排队路径尚未通过；238将在此成功终态后仅扩展原测试至五合同，保留原三合同全部字节语义。正式Rust未apply；18顶层OPEN。


2026-10-09 五项文本冲突静态处置261/262：对原242的RDBC Cargo、data_source/extensions/resource模块和SQLx模块分别核验E preimage、候选及归档原件SHA。Cargo仅新增futures.workspace；两个纯增量mod保留全部原声明/导出；resource十个旧模块与SQLx旧streaming模块均对应247/248已记录条件退休处置，其他原导出名称保留。生成五份完整resolved-review，未写E/F业务源码。zip262 SHA 18023e85361bcb3aa3cb7fbac04392a3ca28f93336183b229eb3594e8213604d，全部条目/E镜像核验。原43冲突中五项取得静态处置草稿，另38项仍需逐项审阅；不是编译、行为或最终集成验收，退休仍须最终门禁。18顶层OPEN。


2026-10-09 三项增量冲突处置263/264：standard_resource_registration仅增加belongs_to/request_cleanup及Arc导入，resource_owner仅增加ensure_open默认钩子，standard_unwrapped仅增加RowSet/RowSetMetaData两variant与对应type_id映射。逐项明确移除新增部分后，使用现有Rust词法器核验其余完整文件token与E原件完全相同；E preimage、候选与归档源SHA前后核验一致。保存三份完整resolved-review，zip264 SHA 820a2dfcd6495b80b58ec7baec201a6b48e3e7b58e928c4fc09866c0e368131a，全部条目/E镜像核验。连同261原43冲突已有八项静态处置草稿，另35项待审阅；保留原代码不代表新增行为正确，未apply、未替代最终编译/行为/API/C1/C2验收。18顶层OPEN。


2026-10-09 外部租约/资源上下文冲突处置265/266：ExternalConnectionLease三个LOB返回签名迁移至Box<dyn标准trait>，原委托体不变，新增SQLXML/Struct/Array直接委托；ResourceContext明确增加owner有效性检查及原错误分类通知。仅逆转这份明确变更清单后，两个完整文件其余Rust token与E原件完全相同；源SHA前后核验。zip266 SHA 7346a964732031713678d345d3b3e1b8aefc2b8005c20c25b48ac4da706a403f，全部条目/E镜像核验。原43冲突累计十项静态处置草稿，另33项待审阅；签名迁移不是SemVer兼容证明，新增行为仍须完整运行门禁。未apply正式业务源码；18顶层OPEN。


2026-10-09 取消后直接与排队失败重试完整五合同终态240：238原235全部五合同5/0、strict0、完整RDBC606/0、fmt0；1275输入变化0，生产代码相对231未变，唯一delta为测试从三扩展至五。zip240 SHA 159dfccf3aec6c043933fc695fb186fbad0e0f05808631e33cc65ab6b197b49c，父239，全部条目/E镜像核验；原235的3/2 RED及234夹具失败保留在237。正式Rust未apply；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 四项结果集/对象冲突处置267/268：ResultSet新增release_owned，ResultSetExt新增owning Stream适配；PreparedInputParameter仅把Struct加入原标量拒绝分支；RdbcObject新增Struct variant及不物化Display。逐项明确排除这些新增部分后，四份完整文件其余token与E原件完全相同；preimage/候选SHA前后核验。zip268 SHA e206f0014b72b4061ae76b910f404e98dc5316a37cf6ac5fcc3150f4fe01be53，全部条目/E镜像核验。原43冲突累计14项静态处置草稿，另29项待审阅；新增游标释放行为与enum API仍须最终runtime/SemVer/C1/C2门禁。未apply正式源码；18顶层OPEN。


2026-10-09 七项资源冲突静态处置269/270：Array/Blob/Clob/NClob/Ref/SQLXML旧值句柄88个公开方法全部在新一对象一文件实现中找到，86签名不变，仅SQLXML source/result显式接纳nullable表示请求；新增validate_for_binding。Factory原六公开名称保留并明确迁移Box<dyn标准trait>参数，新增Struct入口。逐项冻结E preimage、242标准trait稿、新值句柄及240同源生命周期实现SHA；zip270 SHA a73c7753059e5cae7b9b42ea78591f782161492e97b61d395bc2be4e2e3cb0b7，全部条目/E镜像核验。此为方法对应及迁移处置，不证明行为等价或SemVer兼容，旧私有context构造器退休仍依赖正式owner迁移与最终门禁。原43冲突累计21项处置草稿，另22项待审阅；未apply正式源码；18顶层OPEN。


2026-10-09 十七项测试冲突静态处置271/272：按所属函数逐项核验原622个assert宏，除唯一旧Access free调用迁移为标准Array free的明确记录外，原断言token/重复次数均在同名函数保留；未用全文件总数替代所属函数。原函数名仅移除42个Debug fmt helper并改derive，测试名未发现删除。保存17份完整resolved-review，zip272 SHA d7ef80b48c122725480b5a13dfe34247bfd53d1e21f545a15cf5c6423b8130f2，全部条目/E镜像核验。此检查不证明夹具、setup、cfg、可达性或运行行为等价，仍须原完整C1/C2及最终集成编译运行。原43冲突累计38项静态处置草稿，剩五处生产冲突待审阅；未apply正式源码；18顶层OPEN。


2026-10-09 原配置实库253终态RED归档273：PG与MySQL均实际Cargo101/0PASS1FAIL；PG超时丢失标准SQLState，MySQL TIMESTAMP与NaiveDateTime/DATETIME不兼容。1275冻结输入变化0，两自有容器移除并再次inspect确认不存在。258因实库成功前置不成立终态exit1、commands为空，明确NOT_RUN而非全仓测试失败。保存完整原日志、summary、runner、冻结清单及候选/正式四文件preimage；zip273 SHA 8e8d8b37f6b1a7f6e999b24bfac1c7dae13c63c4ca2f0f3871635cc28beb673d，全部条目与E镜像核验。尚未修复或获得GREEN；正式源码未apply，18顶层OPEN。


2026-10-09 实库修正首轮275编译失败归档277：三原命令均Cargo101/E0308，SqlException变体要求Box而候选构造遗漏；实际0PASS0FAIL，断言未执行，不作为产品runtime RED或GREEN。1276冻结变化0，两自有容器清理并inspect确认不存在。保存两生产修正、新MySQL时区/微秒/NULL/缓冲流式合同及全部原日志/summary/runner；原253两个实库RED273仍有效，原完整合同未缩减。zip277 SHA 40201e75d277c1b2640d15718da39ea7c91bc148301648a6c9cf848157c46c90，全部条目/E镜像核验。先前启动归档脚本因原275已终态拒绝LIVE声明，未产出伪启动记录；正式E业务源码未apply，18顶层OPEN。


2026-10-09 四处剩余生产冲突静态处置278/279：Wrapper仅新增三RowSet/metadata继承宏分支；ActiveLease仅将堆栈捕获提前至借用计时之前；PooledResultSet显式增加owner同步handoff与owning Stream清理请求；Registry抽出共享同步入队、身份检查及捕获runtime。逐项逆转明确变更后四份完整原token与E preimage一致，另单独核验原registry入队体仅admission可选化与等待位置改变。zip279 SHA 35ea46f6f9403412699956c2f64f6e92c9f1604dacd2d9a8bf85a5d85cb04abb，全部条目/E镜像核验。原43冲突累计42项静态处置草稿，剩SQLxPreparedStatement实库修正中；不认证新增运行行为、Java计时顺序或最终集成/API/C1/C2。未apply正式/运行中候选源码，18顶层OPEN。


2026-10-09 实库281终态归档283：Box构造补正后PG原完整合同越过SQLState超时断言，实际0/1失败于statement_cancel UnsupportedOperation；MySQL原完整合同越过TIMESTAMP参数往返，实际0/1失败于命名SAVEPOINT走prepared protocol。独立必配MySQL三个会话时区/微秒/NULL/缓冲/流式合同实际1/0；不扩大为完整实库通过。1276冻结变化0、两自有容器移除并inspect确认。zip283 SHA 37064712872d85ac990cb823150f0c8805949b604fd34067a6341c89aae07056，全条目/E镜像核验。下一步控制SQL在MySQL使用text protocol；显式cancel需按原合同及完整生命周期设计，不能用no-op或把cancel偷换为close。正式源码未apply，18顶层OPEN。


2026-10-09 MySQL控制SQL修正285终态287：仅原连接MySQL控制入口从query prepared改Executor &str text；两个原实库合同均实际0/1失败于显式statement_cancel，MySQL已越过原保存点断言，原超时SQLState检查继续通过。独立必配MySQL三个时区/微秒/NULL/缓冲/流式合同再次1/0。1276冻结变化0、两自有容器移除并inspect确认，zip287 SHA 86db25cd73e91ae83bad583bae505841754513eb0ff35f611a36f4e70334b830全条目/E镜像核验。完整PG/MySQL仍RED；不以capability=false、cancel no-op或close替代，下一步定向验证取消与语句/连接复用。正式源码未apply，18顶层OPEN。


2026-10-09 SQLx取消定向RED295归档296：289首次因测试误用set_int编译E0599，不算运行RED；294仅改为实际set_parameter API，原行为断言不变。295实际0PASS2FAIL，均statement_cancel UnsupportedOperation，1277冻结输入变化0。zip296 SHA 529123cec2b7c1ce6c0cb0ec370df4ed6d7cd46e4d507545feb4b77783346e8d全部条目/E镜像核验，保留生产preimage和完整日志。另原113/129/215进程handle已缺失且无成功终态，不再声称LIVE或PASS。正式E源码未apply，18顶层OPEN。


2026-10-09 SQLx取消候选299定向GREEN归档300：原295两个失败合同原字节保留，实际2PASS0FAIL；预取消准入HY008且写入0、下一次执行成功，活动游标停止后语句复用。1277输入变化0，zip300 SHA bc3cdf9442cb7eb98f5c842802cb557399712eb7edbf62147b0db6b78426f855全部条目/E镜像核验。仅F候选增加取消请求与执行代次隔离；尚未重跑完整PG/MySQL，不认证同一借用的并发server-side cancel，不关闭Task7。正式E源码未apply，18顶层OPEN。


2026-10-09 SQLx取消扩展304完整终态308：四取消合同4/0、原异步结果集15/0、指定lib/两测试严格Clippy0、全fmt0；1277冻结输入变化0。新增保留旧游标时cancel自身释放actor、新代次不受旧close影响、重复预取消只消费一次且保留普通批次0写入/重试2写入。302新增夹具unwrap_err要求dyn ResultSet Debug的E0277编译失败保留，303只修错误提取与格式，原两个合同文本保留。zip308 SHA ada622eb47aa01496a62384476262f8d5da2ccd2418ba5cb65d1e55ab288c5e4全部条目/E镜像核验。完整实库/全Wrapper/全仓仍须新鲜后继，不认证并发server-side cancel或关闭Task7；正式E源码未apply，18顶层OPEN。


2026-10-09 实库306重启夹具端口漂移终态312：原PG/MySQL完整合同均越过timeout/cancel进入database-restart；Docker随机发布端口实际59558到60619、61939到54782，原URL仍指向旧端点。保存inspect端口证据与已核验本任务Python->Cargo->测试exe链后，仅停止两个等待旧端口的测试，原exit0xffffffff/0PASS0FAIL，属于夹具中止，不记产品runtime RED或GREEN。后续时间补充合同也指向旧端点，不能做产品认证。1277输入不变，自有容器清理并inspect确认；zip312 SHA 9b828ea5347aefb7df4f9870ca8d6bb8a3807267c08f5d6d3f74d623c2d65db2全部条目/E镜像核验。下一轮只修夹具显式固定随机loopback端口和重启前后endpoint核验，原产品代码/合同不改，完整全仓尚未启动；18顶层OPEN。


2026-10-09 取消候选配置实库315完整GREEN终态317：原PG/MySQL完整合同各1/0，独立必配MySQL时间合同1/0；原超时、显式cancel、坏凭据、类型、事务/保存点、batch、回收/缓存、重启读已提交数据及清理断言保留。仅修夹具显式随机loopback端口，重启前后映射及主机TCP现场核验，原306漂移中止312保留。1277冻结输入变化0，两自有容器移除并inspect确认。zip317 SHA 1bd6b0c3e9850b29d3d44901aa5dc1500e17b6e070780ee4265fe88f3b96c6c6全部条目/E镜像核验。原实库cancel合同为执行前请求，不扩大为server-side并发中断；完整全仓/API/C1/C2仍须新鲜后继，正式E源码未apply，18顶层OPEN。


2026-10-09 当前取消修正候选完整验收319受控启动320：原315三个实库命令各1/0、317归档及清理核验后启动session17147/Python59264，现场OpenProcess证明原进程存活。1277冻结输入一致，同d177主/external缓存串行执行完整workspace/allfeatures/nofailfast、alltargets strict、doc、fmt及原完整C1含C2；不缩减原身份分母或timeout。320启动快照SHA e1b27cb237c4aa643f2be6826d14ca6b27cb84d5ac8dd185e896517acfcfe619与E镜像核验，STARTED_NOT_PASSED。原113/129/215无原handle与成功终态，不能替代此快照。正式E源码未apply，18顶层OPEN。


2026-10-09 最后一处原43集成冲突321/322静态处置：SQLxPreparedStatement明确列出异步查询分流、超时HYT00、取消代次/预取消HY008、九执行入口准入及close停止游标。逐项逆转后完整文件token与242 E preimage完全相同，原方法名全部保留，仅新增cancel/execute_query_values；1277冻结候选及E preimage前后核验一致。zip322 SHA fc421dbd8ef499be4063ba37a500b06c8992351d30f9a26d35649a4223d69606全部条目/E镜像核验。原43冲突全部取得静态草稿，不等于集成/编译/行为/API/SemVer认证；当前319全仓验证仍待终态，正式E源码未apply，18顶层OPEN。


2026-10-09 旧clean适配器审阅刷新324/325：SqlxConnectionAdapter相对242旧候选仅两处MySQL实库修正，分别保存点无绑定text protocol与已确认TIMESTAMP类型的NaiveDateTime解码。逐项逆转后完整旧候选token相同，E preimage与1277冻结前后一致；317完整实库及三时区微秒/NULL/缓冲/流式补充已通过。zip325 SHA 78bae53dc3a55c90f2cf0d2a91dc19cf0a8a14e1c0444a592bea41f9512787c9全部条目/E镜像核验。旧244 clean SHA已过时，最终集成必须使用本轮desired SHA；不认证最终API/allfeatures，正式源码未apply，18顶层OPEN。


2026-10-09 当前冻结候选集成清单刷新323/326：基于319的1277输入，逐项保护1257源码/支持路径E preimage，状态{"IDENTICAL": 1090, "DIFFERENT": 68, "MISSING_IN_E": 99}；已有静态审阅匹配当前desired SHA 55项，过时审阅0项，E-only 11项保留条件退休及替代源当前SHA。候选较236变化6项，E较236源码变化0项。zip326 SHA 3855db3e76dcb897bf5bdba1aea42625114e7b07725914e3ea389ccd02811768全部条目/E镜像核验；这是审阅/保护清单，不是apply或验收，319全仓仍待终态，正式业务源码未改，18顶层OPEN。


2026-10-09 当前冻结候选最终语义门禁327实际exit1：第2行PLANNED_TARGET拒绝。328逐行调用原状态策略，69行中61行拒绝，8行仅状态可接受；目标未声明、adapter/logical/capability未绑定证据逐项列入修复矩阵。未改状态/来源/验证器，不将候选源码或import通过当最终类型/运行证明。1277冻结输入前后不变，zip329 SHA 8bc503c0a5802661293abb5e63256c2eb41d1580df11093421b70a27f82a0b17全部条目/E镜像核验；319全仓仍独立运行，Task8 INCOMPLETE/18顶层OPEN。


2026-10-09 当前冻结1277候选形态复核330/331：五个异常payload对象为struct但清单ENUM_KIND，DriverManager映射DriverRegistry struct但清单MODULE；六项逐一源码与pub use重导出SHA绑定。仅静态冲突证据，不是compiler/API或最终门禁通过。批准异常kind+payload和manager module语义须保持，不放宽验证器、不提升LIVE；候选输入前后变化0。zip331 SHA 2081e072b1f6d44db9e697c9d7a1670ffbdd0e72b4a3a14847b51416506f1e66全部条目/E镜像核验。319原完整全仓仍运行，正式E源码未apply，18顶层OPEN。


2026-10-09 五异常形态草稿332/333：仅五个现有struct payload的rust_shape由ENUM_KIND拟改DTO，其余18列逐字保留，69行/52+17Java来源不变；原load_oracle和verify_final_source_binding对原件及草稿均通过且冻结摘要一致。没有状态提升、验证器修改或生产apply；DriverManager MODULE/固定DriverRegistry入口冲突保留。1277输入变化0，zip333 SHA 2ea486505aca74eb076840a1018c217b60d00a7fec3ef460e1fa1c01226f23ea全部条目/E镜像核验。新鲜compiler API与运行证据仍必需；319原全仓继续，18顶层OPEN。


2026-10-09 六canonical形态草稿334/335：五异常payload及已批准固定DriverRegistry类型入口均保留，只有六行rust_shape拟改为实际struct的DTO；其余18列逐字保持、69行/52+17Java来源绑定通过。原形态策略对源码声明fixture六项逐行拒绝原形态、接受草稿形态，此为静态fixture而非compiler API认证。没有状态提升/门禁放宽/源码apply，规格第8节固定registry extension入口与原Task3映射保持。1277输入变化0，zip335 SHA b65119a4b6cbf2adaa5757189a62789cd0b1a0696bc0277ef79d97dd9d4a125b全部条目/E镜像核验。新鲜compiler API与运行证据仍必需；319原全仓继续，18顶层OPEN。


2026-10-09 原最终门禁防护自测336/337：七个来源绑定/身份/分母/摘要/声明篡改检查与两个API种类/alias/import不足检查，原九测试实际exit0；不改原测试或验证器、不启动Cargo、不作为最终API/runtime验收。1277冻结输入变化0，zip337 SHA b6cf34a35658726d20d3967c99eaa8d8124ed9816b22a413940398fcab7a727d全部条目/E镜像核验。六形态草稿335尚未apply，61状态拒绝仍开放；319原全仓继续，18顶层OPEN。


2026-10-09 69行源码形态预检338/339：针对原清单和未apply的六形态草稿，逐项绑定直接声明/枚举成员位置及SHA，状态 {"DRAFT_SOURCE_KIND_MATCH": 67, "EXCLUDED_HOST_RUNTIME": 1, "UNRESOLVED_SOURCE_DECLARATION": 1}；直接源码查找不证明公开可见性/cfg/alias/compiler/API/runtime。1277冻结变化0，zip339 SHA 00d45ebae9fd895c4be6e44228fd2f46031f3772db64449dbd332acb35656c37全部条目/E镜像核验。未提升状态或改原门禁，319原全仓仍运行，18顶层OPEN。


2026-10-09 69行重导出形态补证340/341：针对原清单和未apply的六形态草稿，逐项绑定直接声明/枚举成员位置及SHA，状态 {"DRAFT_SOURCE_KIND_MATCH": 68, "EXCLUDED_HOST_RUNTIME": 1}；Savepoint显式connection->types重导出两级及源声明已SHA绑定；339直接查找未解析不表示产品缺失。源码预检仍不证明cfg/compiler/API/runtime。1277冻结变化0，zip341 SHA 4d5bd391b4566ba775eb9f53514f59c7f3bc085d0f6e6d394dbdbbe98b48706c全部条目/E镜像核验。未提升状态或改原门禁，319原全仓仍运行，18顶层OPEN。


2026-10-09 原完整验收319原生线程诊断342/343/344：直接系统进程快照验证Python59264/Cargo26048父链及本任务rustc线程数值状态，保留完整原始记录；Ready只是调度诊断，单快照不证明根因/死锁，不以无新增日志或低CPU认定终态。1277冻结变化0，未改优先级/停止/重启原任务，zip344 SHA 48d33dc319b36b0a75eef622923cb59771d7554cd9d1dc26b0668de1263a65e8全部条目/E镜像核验。仍STARTED_NOT_PASSED、18顶层OPEN。


2026-10-09 原全仓319调度环境345/346：仅现场核验PID/父链/完整1.97.1映像路径/创建时间的本任务Cargo及rustc从BelowNormal设为Normal，逐项保留原优先级及10秒CPU观察；未改全局策略、其他进程、源码/原命令范围/超时/断言，未停止或重启任务。1277冻结变化0，zip346 SHA c1e975121171d101d7e6e9f87e2613e56dbbbd6fa42d2de35883d6fb577e8875全部条目/E镜像核验。观察不是性能或产品PASS，319仍待终态，18顶层OPEN。


2026-10-09 当前105条compiler coverage源码预检348/349：原七家族每家15义务分母保留，源码hash统计 {"STALE": 105}；原manifest加载和canonical源码预检结果 [{"name": "original-manifest-load", "status": "FAIL", "error": "coverage requirement source hash is stale: crates\\druid-wrapper\\tests\\duckdb_connection_adapter_test.rs: 821966759410c319a42100f055b6fb28135bfb814b25920c0a1ef189fe81da87 != 4a43bca0123a34fb6468a020ae9891e182e9f37f7f101647417dff267ee6e78f"}, {"name": "canonical-source-preflight", "status": "FAIL", "error": "SQLx actor ownership evidence is incomplete: ['tokio :: sync :: Mutex']"}]。未启动Cargo/修改hash或编译器identity，不认证runtime coverage；必须在319终态后用原七精确测试新LLVM profile重生成105义务。1277冻结变化0，zip349 SHA b0aece9956ebd516381990e6dc767e326f1b34465884418b16292e29a9ee6ca1全部条目/E镜像核验，Task6 FIX_REQUIRED/18顶层OPEN。


2026-10-09 SQLx所有权校验草稿350/351：原源码组合导入RED，显式Mutex导入后暴露原校验器引用已退休Streaming文件；未应用草稿将原缓冲检查迁移到Buffered并额外拒绝Async丢失actor/扩大容量/绕过pull或worker/不等待关闭/保留Vec<Row>。13项原RED/草稿GREEN及负例均符合预期；完整105源码预检 {"status": "FAIL", "error": "runtime trace record callsite multiset drift: missing={('crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs', 'task4_trace_result_next'): 1, ('crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs', 'task4_trace_result_set_metadata'): 1}, extra={('crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs', 'task4_trace_result_set_metadata'): 1, ('crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs', 'task4_trace_result_next'): 1, ('crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs', 'task4_trace_result_set_metadata'): 1, ('crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs', 'task4_trace_result_next'): 1}", "requirements": 105}。无Cargo/编译器或runtime coverage认证，1277冻结变化0，zip351 SHA 8e6fe1989b718595ff9a3c9f0a22c7f4d7fbba9ad4d7a1e5ffafc547bcab2e1b全部条目/E镜像核验。Task6及18顶层仍OPEN。


2026-10-09 SQLx所有权/trace草稿352/353：保留原15x7义务和所有actor/update/metadata检查，Buffered绑定真实缓冲路径，额外登记Async两个各恰好一次调用点；完整105源码预检PASS_STATIC_ONLY，8个缺失/重复调用负例拒绝。原4测试3PASS/1HTTP fixture错误，用原校验器复现；仅限制fixture调用选择到规定函数范围后4/0，错误函数负例保留。原精确runtime来源仍只允许Buffered并拒绝退休/Async替代。合成来源数据不作为runtime，未修改过期hash/compiler身份、无Cargo，1277冻结变化0，草稿未apply。zip353 SHA ea8f25a3872a8abcb41f643cb38e0718fa92b3987d23643fc5cb88c83c311cb7全部条目/E镜像核验；105新鲜LLVM覆盖及319终态仍待完成，18顶层OPEN。


2026-10-09 SQLx校验器完整原suite草稿354/355：全部原测试保留，结果 44 tests/0FAIL/4ERROR/1SKIP；新增两个方法承载异步所有权7负例和双结果集trace8负例。旧fixture路径迁移与精确函数选择补丁完整可审阅；错误/环境问题保留，不替换为局部PASS。无Cargo/LLVM认证、未apply，1277冻结变化0，zip355 SHA 54890c1c52078a86200208cbb7a01b7ca7c1c8b7562583f1947b8918f9b1e5c1全部条目/E镜像核验。18顶层OPEN。


2026-10-09 Windows有界执行器草稿356/357/358：原WinError10038后新增Windows私有Job分支，挂起创建->先归属->恢复，9项原生实测通过（4项后代清理），总输出/deadline保留；原未改动bounded-runner测试 False，全部错误日志保留，POSIX实现未改。无Cargo/LLVM/产品runtime认证、未apply，1277冻结变化0，zip358 SHA 4d534a05ca950c425929df037ac783b0dac3073e2b2019b55bba9408c5b89cd8全部条目/E镜像核验。18顶层OPEN。


2026-10-09 Windows执行器边界/故障359/360：8场景实际通过；恰好输出cap与+1拒绝、关闭管道仍保留0.1s超时、259退出码、环境/Unicode字面参数、3次创建失败句柄计数不增；真实挂起子进程归属失败注入证明未执行用户代码且私有进程/句柄已清理。存活探测只用同步/查询句柄；原测试python3为WindowsApps别名，原suite仍不PASS。无Cargo、未apply，1277冻结变化0，zip360 SHA 4398da8449d3a961e1830184b2599809e5559db9ce42dd9b020d537c24c0afde全部条目/E镜像核验。18顶层OPEN。


2026-10-09 Windows测试fixture草稿361/362：仅改解释器到sys.executable和只读OpenProcess/WaitForSingleObject存活探测，保留原0.1s超时/2s elapsed/4096及1024输出cap/后代退出断言；原测试实际PASS=False，全部错误/失败保留。未增加超时或删除断言、无Cargo、未apply，1277冻结变化0，zip362 SHA 8bb900b11cd22f3ab746ac301a671b278cf9fed0b81ed25c11bf0c2ca20757d3全部条目/E镜像核验。18顶层OPEN。


2026-10-09 Windows运行deadline对齐草稿363/364/365：原WinError10038后新增Windows私有Job分支，挂起创建->先归属->恢复；与原POSIX一致在launch/pipe就绪后开始原deadline，9项原生实测通过（4项后代清理），总输出/deadline保留；保留原0.1s/2s/output/后代断言的解释器与只读探测fixture测试 False，全部错误日志保留，POSIX实现未改。无Cargo/LLVM/产品runtime认证、未apply，1277冻结变化0，zip365 SHA f458dfae22c34165b795b259424507487fa00b6337d011893122cd976c0b2825全部条目/E镜像核验。18顶层OPEN。


2026-10-09 清单单元fixture草稿366/367：三个负例使用临时7x15清单和UNIT_TEST_ONLY_NO_LLVM标识，错hash/家族/重复顺序断言保留；全部原42+新2测试结果 44 tests/0FAIL/1ERROR/1SKIP。生产清单SHA不变且原加载仍拒绝过期hash，105覆盖未认证；Windows原0.1s/2s/输出/后代断言未缩减。无Cargo、未apply，1277冻结变化0，zip367 SHA 6dbf1d20b250f0ba76bed0ff9122ff5ce293ecabefb5cfc9002c7f27e2a1b53d全部条目/E镜像核验。18顶层OPEN。


2026-10-09 原0.1s测试私有Job诊断368/369：原测试PASS=False；清理点只读成员记录 [{"elapsed_seconds": 0.3129999999946449, "root_pid": 39004, "pids": [39004, 17696], "descendants": [17696], "assigned": 2}]。保持原私有Job Term、超时与断言，不探测/终止外部进程，无Cargo、未apply，1277冻结变化0，zip369 SHA f2e0f3d69f9a6e42ea1bd16c98f5dd409f54bb829a4fc429dda5186ba94943e2全部条目/E镜像核验。18顶层OPEN。


2026-10-09 私有Job终止证据草稿370/371/372：Windows测试在原0.1秒超时错误中要求恰好一个归属已验证并已终止的后代；文件存在时仍逐项核对PID；三个清单负例使用临时7x15及UNIT_TEST_ONLY_NO_LLVM标识，错hash/家族/重复顺序断言保留；全部原42+新2测试结果 44 tests/0FAIL/0ERROR/1SKIP。生产清单SHA不变且原加载仍拒绝过期hash，105覆盖未认证；Windows原0.1s/2s/输出/后代断言未缩减。无Cargo、未apply，1277冻结变化0，zip372 SHA 4e3a15f0eac32072069e313410e5c6d5dea57536dfa606e018502a5dda295d8d全部条目/E镜像核验。18顶层OPEN。


2026-10-09 当前门禁草稿最终补证377/378：原覆盖生成器全部7测试PASS；最新370 helper复跑9 native与8边界/归属故障场景PASS；原373句柄197到193严格相等失败保留，复核拒绝净增长并逐个跟踪故障路径私有句柄均关闭；原105源码义务预检PASS_STATIC_ONLY。父372原42+新2测试43PASS/0FAIL/0ERROR/1POSIX-SKIP。原生产manifest仍拒绝过期hash，无新LLVM/产品runtime认证、无Cargo、未apply，1277冻结变化0，zip378 SHA 8e2327c549ce291cb2e66db2ce09faf37d37dc495640c0f144d5392d7650b93f全部条目/E镜像核验。18顶层OPEN。


2026-10-09 四文件门禁补丁审阅379/380：真实F候选git apply --check exit0，三个Python AST PASS；逐项绑定F/E preimage与desired SHA，正式E集成尚未认证。373句柄197→193相等失败及375测试代理ctypes签名失败保留，377故障路径27创建/27关闭/0遗留。原生成器7/native9/故障8及完整suite43PASS/1POSIX-SKIP证据绑定；生产105LLVM清单仍拒绝过期hash、未apply，1277冻结变化0。zip380 SHA c71b2840f5c5e04acef2bbb7d0d08d01123a0c610ec76f76096ac790714aee0f全部条目/E镜像核验。原319继续，18顶层OPEN。


2026-10-09 剩余13文件静态集成审阅381/382：12个E已有未跟踪文件及1个HEAD原件逐项保存preimage/desired/diff；原方法无删除，62/65断言逐token保留，3个LOB身份断言迁移为同Box数据地址并补3个长度断言。两文件完整逆变换相同；SQLXML名称OUT独立对象迁移明确保留运行待验收边界。68个差异源码均有静态草稿，不等于行为/API/正式集成通过；1277冻结变化0、无Cargo、未apply。zip382 SHA 0ccbc69eac533059c26f7aca2ba111cb09ce15c23b2c6032b113433956b6e966全部条目/E镜像核验，319仍继续、18顶层OPEN。


2026-10-09 当前冻结候选集成清单刷新383/384：基于319的1277输入，逐项保护1257源码/支持路径E preimage，状态{"IDENTICAL": 1090, "DIFFERENT": 68, "MISSING_IN_E": 99}；已有静态审阅匹配当前desired SHA 68项，过时审阅0项，E-only 11项保留条件退休及替代源当前SHA。候选较236变化6项，E较236源码变化0项。zip384 SHA 186947fa6617ecf542428e8091cd940b8304e897cd9eae2100ba5e4ed2c84a41全部条目/E镜像核验；这是审阅/保护清单，不是apply或验收，319全仓仍待终态，正式业务源码未改，18顶层OPEN。


2026-10-09 新增99文件接线预检385/386：96个Rust文件有真实Cargo lib/main/默认tests根至mod/path的逐文件SHA路径，三Python AST有效，解析到的声明缺失0项；仅静态接线，不证明cfg/compiler/public API/runtime。无第二Cargo、未apply/退休E文件，1277冻结变化0。zip386 SHA 5c88cfb06e25be17acdeabfd25133edee7be6c0af4c1e418beb709dd0ffa05f3全部条目/E镜像核验，319继续、18顶层OPEN。


2026-10-09 集成前只读检查器387/388/389/390：有效预览初始RED，补 exact1277/1257分母及原5命令scope、逐日志SHA/C1 executed=yes、源码/E preimage/新路径/path检查后21测试PASS；当前真实319 {"state": "REFUSED", "reason": "verification still running"}，未apply/退休/认证LLVM/API，1277及1257正式preimage不变，无第二Cargo。zip390 SHA 2bd48094de8984231aa2bc08143dc051da7e8dee6c52eeb1b1fa81ba08fd8c18全部条目/E镜像核验；18顶层OPEN。


2026-10-09 原319完整workspace/all-features测试终态391/392：原命令含locked/offline/-j1/no-fail-fast实际exit0，4198 PASS/0 FAIL/0 ignored，原1277冻结输入hash逐项不变，日志SHA b83fb484decc9ffe88d486699986d46d667f1d28fb9c29d76642845b70e8d66d。原执行器继续strict/doc/fmt/完整C1-C2，不能提前声明其他四门禁PASS；未apply正式源码，LLVM/API/MSRV及18顶层任务仍OPEN。zip392 SHA e2611291729e76f8886a8dc9cd0bbee18c0cfd25a9a9d0f9592b6cbc16306705全部条目及E镜像核验。


2026-10-09 集成检查器可执行文件身份草稿393-397：替换Cargo/Python但保留参数及日志hash两个负例实际RED，原21项与扩展23项共44测试GREEN；独立草稿绑定原1.97.1 Cargo与Espressif Python绝对路径，原检查器保留。396实际预览仍拒绝运行中的319，1277候选输入与1257正式preimage复核不变；未apply/认证API/发布，18顶层OPEN。zip397 SHA a5b75c29053c77af9a76849b3ab7c306782db679934fd2329f6b93d9d32aba04逐条目和E镜像核验。


2026-10-09 原319四门禁终态399/400：全仓allfeatures测试4198/0 exit0；strict/alltargets/-D warnings实际exit101，lifetime_tests.rs两处unchecked_time_subtraction；doc/no-deps与fmt/all/check实际exit0。两处checked_sub草稿rustfmt通过、逆变换逐字节相等，原年龄/阈值/断言保留，未apply或宣称修复后Clippy/runtime GREEN。1277冻结变化0；原完整C1/C2继续。zip400 SHA 1c663921247ef1ac0b01281bdf6cfb153e26060ebf080a4fba84fe5ee4b759c8逐条目及E镜像核验，18顶层OPEN。


2026-10-09 原319完整终态及候选修复402：原5命令exit [0,101,0,0,1]，全仓4198/0；C1因C2 wrapper/all-features/lib原1800秒超时失败，替代测试NOT_RUN/ERROR，不判行为PASS。1277原冻结输入终态一致，完整zip402 SHA f337aa5de597459f9ec40c9d9e4d5afc7811aba74a6e8eeeb9880f994daeb971逐条目/E镜像核验。终态后仅在F候选应用已审阅四文件门禁补丁及两处checked_sub测试修复；正式E源码未集成，修改后Clippy/runtime待实测。原超时/分母/断言保留，18顶层任务OPEN。
`n2026-10-09 修改候选验收403：402补丁应用后完整wrapper门禁自测44项实际exit0，43PASS/1POSIX-SKIP。新冻结1278输入；原wrapper/offline/locked/all-features/lib/show-output命令正在真实预编译，后续串行执行原完整workspace/all-features/no-fail-fast、strict/all-targets/-D warnings、doc、fmt和完整C1/C2；保留C1/C2原超时和分母。session56719/Python67796，尚无终态，不记PASS；正式E业务源码未集成，18顶层OPEN。


2026-10-09 原wrapper预编译403终态：原offline/locked/all-features/lib/show-output命令exit0，18PASS/0FAIL/0ignored，1563秒，日志SHA 8126a1766e7f00d5a9d309b8724c220318fcc30da1545518603d354bd97b0943，1278冻结输入变化0。linker warning保留；原319 C2超时失败仍保留，不宣称根因已解决或完整C1/C2通过。同一session56719继续全workspace/all-features及strict/doc/fmt/完整C1-C2，正式E源码未集成，18顶层OPEN。


2026-10-09 当前403集成前检查404：旧393检查器针对新1278输入实际拒绝分母不匹配；独立新检查器绑定原403执行器SHA、319终态来源、新六条精确命令与1258集成路径，保留完整C1/C2、479/442/executed=yes、逐日志SHA和原文件保护。原21项、身份扩展23项与新29项共73测试PASS/exit0。1257正式原文件hash及1278候选冻结输入逐项不变，新helper正式路径未占用；当前状态{'DIFFERENT': 71, 'IDENTICAL': 1087, 'MISSING_IN_E': 100}。五个402变更路径需按当前desired SHA集成；原383静态审阅不得冒充更新后的行为认证。真实403预览仍REFUSED：verification still running。仅外部检查器/保护清单更新，未集成/退休正式源码，不认证LLVM/API或发布；18顶层OPEN。zip404 SHA 2405aba91bf1061a3c9921cef3655d91019f8eb52942e852559d7916537d4974全部条目及E镜像核验。


2026-10-09 验证材料源码公开调整：404 检查器、测试及证据已逐条 SHA 核验后展开为 docs/verification/rdbc-first/preflight404/ 普通文件。原 ZIP 留本地，后续 SDD ZIP 不再新增到 Git；历史已跟踪归档保留引用与提交历史。该目录是工具/证据快照，不是正式候选集成或18任务完成。


2026-10-09 修改后403完整Workspace终态405：原workspace/all-features/locked/offline/-j1/no-fail-fast实际exit0，4198PASS/0FAIL/0ignored，日志SHA 7805a88c67bbadba895f9d8622a492149e64a3ddc123df7763094a7bf8e1e233；日志重新统计与原结果一致，1278冻结输入逐项不变。本轮来源包含402候选修复，不能与旧319混用。证据及执行源码以普通文件保存在docs/verification/rdbc-first/workspace405/，不新增ZIP/PDB/编译产物。同一session56719继续严格Clippy及doc/fmt/完整C1-C2；这些后续门禁尚未取得整轮终态。正式E业务候选未集成，LLVM/API/SemVer/MSRV/feature/实库全矩阵及18顶层任务仍OPEN。
