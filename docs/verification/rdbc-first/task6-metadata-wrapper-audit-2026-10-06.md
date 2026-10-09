# Task 6：元数据 getter 与 wrapper 身份审计（2026-10-06）

本轮延续完整 P04 Tasks 6–8 与 P05–P07 目标（18个顶层未闭环任务）。前一轮真实恢复两个 object 身份案例并验证 RDBC471/Druid139，通过固定1283 BASE账本将最低缺分支57降为55，属于进展。本轮不把已通过的普通 getter 转发替代完整 metadata 身份合同。

## 权威来源

- 固定 BASE：5afe8609b177544ff7d18461aa84f69efe8ee90e。
- 原案例：crates/druid/tests/druid_pooled_result_set_semantics_test.rs#physical_result_set_metadata_preserves_getter_error_and_wrapper_identity。
- 当前标准：crates/druid-rdbc/src/metadata/result_set_meta_data.rs。
- 当前代理：crates/druid/src/core/result_set_meta_data_proxy.rs。
- 标准 wrapper：crates/druid-rdbc/src/wrapper/wrapper.rs、standard_unwrapped.rs。
- 已批准迁移：task6-migration-authority.tsv 将 PhysicalResultSetMetaData 合并到唯一标准，将旧 ResultSetMetaData 合并到 snapshot；task6-public-contract-allowlist.tsv 批准旧 from_physical/physical raw accessor 删除。不能据此推断所有 getter 和底层对象身份观察也获准删除。

CodeGraph 返回当前代理各 getter 的原样转发和标准 trait 的默认 snapshot 派生路径。本轮另读取固定 BASE 原案例及 PhysicalMetaDataProbe，未用当前矩阵占位替代原源。

## 原案例完整观察

1. 对列1的全部 boolean、nullable/nullability/code、signed、display size、label/name/schema、precision/scale、table/catalog、typed kind/type code/type name、writability/class name 返回原始值。
2. 保留原24次底层调用的严格顺序；nullable三种表达均进入 nullability，type kind/code均进入 column_type；不能用预读 descriptor 或静态快照掩盖调用和错误路径。
3. 包装器自身、底层标准接口及具体 metadata probe 的类型/解包行为；两次指针比较必须证明同一底层对象，不能比较两个独立 snapshot 的相同数据。
4. clone 相等、不同底层/默认 metadata 不等、Debug 包含实际底层类型。
5. column_label(9) 必须保留准确 DriverError("metadata probe failed at column_label")，底层仅调用 column_label 一次。

原 source identity、body SHA、assertion/call inventory继续以固定账本为准。本轮不变更该行的 MERGED_PARAMETERIZED 状态，不删除任何原始身份断言，不声明 exact 迁移。

## 完整恢复草稿与当前输入边界补充

后续 goal turn 前置为 verified wait：同一完整workspace session41767 多次确认LIVE，cargo51544及其rustc/link子进程继续推进。当前生产crate编译输入未修改；缓存无法finalize的os error5提示不是运行终态，不能重启或冒充测试失败。

已从固定BASE生成未接入Cargo的两个真实草稿：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/meta_data_identity_probe.rs` 和 `metadata_identity_tests.rs`。草稿生成 session32511 终态exit0，原观察 `assert:17;assert_eq:18;assert_ne:2;matches:1` 全部保留，新目标为同样原断言加一项标准视图匹配，调用数量也达到原门槛。首次草稿sanity要求整个计数串相等，因新增matches计数而拒绝；最终检查逐种原断言不减少及原总调用数量，不修改正式验证器或BASE。草稿尚无编译/运行证明、不更新迁移账本、不关闭元数据合同。

当前标准ResultSetMetaData与私有代理均缺少旧对象的is_nullable/nullable_code/rdbc_type派生入口，CodeGraph源码及精准符号核查已确认。完整恢复必须让前两者继续进入原nullability入口，type code继续进入原column_type入口，以维持24次严格调用顺序；不能由ColumnDescriptor预读或静态snapshot替代。记录型草稿明确拒绝聚合descriptor调用，以暴露这种绕过。

生产元数据入口目前接收Arc<dyn ResultSetMetaData>，类型已被擦除。标准WrapperExt只检查Self，不能恢复被擦除的具体后端；泛型私有后端方案尚须证明同一生产包装逻辑及真实底层身份，不能仅新增测试专用构造器就宣称恢复。原精确column_label(9)错误、两次指针观察、clone/Debug/相等性全部保留。下一阶段在当前完整回归终态后接入完整案例，先记录编译或运行失败，再实现私有身份和派生入口，公共canonical继续不暴露Any/Physical/第二驱动树。

## 当前证据与差异

ResultSetMetaDataProxy 已直接调用 raw 的各标准 getter，因此具备保留 getter 错误和顺序的实现基础。ResultSetMetaData::column_count 是不可失败的 usize 标准合同；原 fixture 的该入口在此案例不注入错误，但其他 column getter 的错误仍必须精确保留，不能扩大为“metadata getter都不失败”。

代理当前使用 impl_standard_wrapper!(ResultSetMetaDataProxy => ResultSetMetaData)。标准宏在该目标上返回代理自身的标准视图，而原案例的底层接口 unwrap 指针应指向 raw。此为源码身份差异，尚未新增运行时 RED 证据。

WrapperExt::unwrap_ref/is_wrapper_for_type 的现有实现与中文文档明确只检查 concrete Self；其默认实现不能解包到代理内部的具体 probe。标准 Wrapper 的结构化视图仅包含标准接口，未提供任意 Any 对象。该差异不能以标准目标名替换 concrete 目标后就标为全案例完成，也不能仅为了测试通过重新引入 Physical 或另一套驱动树。

标准 metadata 是 owned immutable Arc snapshot，而原对象曾承载 physical backend。规格明确要求拆分标准 trait/snapshot 与 concrete backend。下一实现应先保留完整原 getter/error/identity观察，通过适用的 Druid-private backend/identity实现及唯一标准视图完成；具体类型部分必须取得真实等价证据或已有明确批准的逐项处置，不能使用测试专用伪装路径。

## 等待与范围

当前全workspace为同一 session41083，task6-object-label-workspace.log。本轮多次通过句柄确认LIVE，并由 Win32父子进程确认 cargo64188 -> rustc60504 -> link71424 正在执行。未重启、取消或杀进程，未在编译期间修改Rust。暂时无工具输出不能视为终态；编译日志不能视为全量通过。

前轮定向 RDBC471/Druid139仍仅覆盖已执行范围；前一已结束全workspace3697PASS/2FAIL早于当前源码。当前55最低缺分支（buffered17/lease2/recycle12/filter24）、C1/C2/Callable5vs6、对象Filter/LOB统计、Task7八组件、Task8及P05–P07继续未闭环。无新API、无安装、分支、提交、推送或发布。下一步先取得同一全workspace终态，再执行完整 metadata 案例的TDD恢复。

## 完整案例实际接入 TDD（2026-10-06）

Buffered修复后的同一Druid回归39062已终态exit0/28PASS/0FAIL/0ignored。之后才修改Rust，避免验证期间修改源码。

完整metadata草稿已接入 `crates/druid/src/core/result_set_meta_data_proxy/metadata_identity_tests.rs` 及同目录 `meta_data_identity_probe.rs`；主对象文件仅增加cfg(test)私有子模块。24getter顺序、17 assert/18 assert_eq/2 assert_ne以及原错误匹配仍保留（额外增加标准视图matches，现matches2）。没有简化为snapshot测试、替换两个原指针观察或降低迁移验证器门槛。测试连接同一生产代理new入口，泛型具体身份需求尚未实现。

完整案例定向构建已启动同一42531，日志 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-metadata-full-contract-red.log`；当前仍编译，不宣称已经取得RED终态或运行通过。下一步读取实际compiler/runtime失败，再实现private getter与identity。公共类型擦除入口的生产等价性仍独立待证，私有测试通过也不能直接关闭全元数据门禁。

迁移账本不变；完整18任务/Task6 FIX_REQUIRED仍保留。

## 实际 RED 和私有代理恢复实现（2026-10-06）

完整案例构建42531已终态exit101，日志task6-metadata-full-contract-red.log：17个编译错误，包含具体Arc构造类型不匹配、缺少is_nullable/nullable_code/rdbc_type、私有具体身份入口、clone/Debug/PartialEq/Default。未修改断言来规避错误。

ResultSetMetaDataProxy 已改为私有泛型后端，默认仍为dyn标准metadata，现有生产调用继续使用同一个new入口。派生入口直接调用raw.nullability/column_type，不预读ColumnDescriptor；JDBC数字映射取自固定BASE result_set_column_type.rs，Integer保留-5，Float8，Binary-3等原映射。clone共享原Arc及attributes；相等性检查原对象地址，不把vtable当后端身份。具体downcast只在Druid-private未擦除后端上实现，标准接口不增加Any/Physical/第二驱动树。

标准metadata解包现在返回raw标准视图，不再返回proxy自身；独占可变后端仅在Arc唯一所有者时提供，不能把共享raw伪装成独占原对象。完整测试两个指针观察保持原样，尚未得到运行结果。

同一完整测试重跑38536仍LIVE，日志task6-metadata-full-contract-green-attempt.log；未经终态不称GREEN。当前标准纯度检查exit0/44/final、格式及diff检查通过。具体后端的生产擦除入口等价性、回归及完整C2仍待证明；账本1092/191及BASE未变，18顶层任务保持开放。

## 首次完整运行 RED（2026-10-06）

同一38536终态exit101，task6-metadata-full-contract-green-attempt.log：实际运行1例，0PASS/1FAIL/0ignored，87filtered。编译已通过；失败在metadata_identity_tests.rs:148，原标准接口std::ptr::eq比较。此前24个getter值及严格顺序、标准视图role匹配、private Self/标准/具体类型检查及具体后端指针比较都已经执行通过。精确column_label(9)错误、clone/Debug/不同对象相等性位于该断言之后，尚未执行，不能宣称已通过。

源码 canonical_unwrap 返回同一raw Arc引用，而此前具体指针比较已通过；标准trait胖指针可能因虚表不同而不相等，但当前仅为待证推断。完整原指针断言保持原样，只新增数据地址/胖指针比较诊断输出。新定向诊断同一97873 LIVE，日志task6-metadata-standard-pointer-diagnostic.log。先取得实际地址证据，再决定忠实的Rust身份观察映射；不预先删除指针观察或替换为snapshot内容相等。

迁移账本及完成门禁不变，18任务仍开放。

标准接口身份映射依据：同一诊断97873终态exit101/0PASS1FAIL，实际 observed=expected=0x1d9df07abf0，data_eq=true/fat_eq=false。原底层对象一致而编译生成的虚表不一致。标准role继续由StandardUnwrapped::ResultSetMetaData检查；两个原指针观察均保留std::ptr::eq，其中标准接口视图明确比较数据指针，不将虚表地址当对象身份，不替换成snapshot内容相等。诊断日志task6-metadata-standard-pointer-diagnostic.log永久保留；临时打印移除。精确错误/clone/Debug/相等性仍待后续运行，不按前置通过推断。

完整private具体后端案例65109终态exit0，task6-metadata-full-contract-data-identity.log：1PASS/0FAIL/0ignored/87filtered。包含先前失败点后的精确column_label(9)错误、clone/Debug/不同后端相等性，均实际执行通过。两个原对象指针观察仍在，standard角色匹配+真实数据地址代替不可靠虚表身份，地址诊断原0x1d9df07abf0/data_eq=true/fat_eq=false证据保留。

此结果尚不证明生产Arc<dyn ResultSetMetaData>擦除入口的具体后端识别和Debug。新增task6_metadata_erased_standard_entry_preserves_backend_identity，使用与生产完全相同的擦除输入和new入口；先验证标准原对象地址、getter及其真实调用，再要求具体后端身份和Debug。迁移账本不更新，不将private路径成功替代完整生产要求。后续先取得该补充案例的实际RED/GREEN。

## 类型擦除入口实际 RED 与标准恢复（2026-10-06）

同一25908终态exit101，task6-metadata-erased-entry-red.log：0PASS/1FAIL/0ignored/88filtered。生产等价的Arc<dyn ResultSetMetaData>输入先通过标准原对象地址、column_label真实值及调用顺序，然后在具体MetadataIdentityProbe身份检查失败；Debug检查尚未执行。private完整具体案例65109的1/1通过不能替代此要求，因此未绑定账本。

按原backend-bearing metadata拆分义务继续恢复：标准ResultSetMetaData增加std::fmt::Debug约束，metadata Wrapper宏自身TypeId目标仍返回同一StandardUnwrapped::ResultSetMetaData视图，不新增Wrapper方法、Any或第二驱动树。私有代理类型检查经同一raw Wrapper查询，标准解包保持raw身份，Debug使用真实raw对象。此为公共约束/目标行为变化，后续必须验证外部编译消费者、API、完整crate/metadata/Wrapper回归；不能以纯度或私有测试替代兼容性证据。

完整private原案例+erased入口两例已用同一47496启动，日志task6-metadata-erased-and-full-regression.log；仍LIVE，等待compiler/runtime结果，不声称GREEN。最新标准纯度已返回44/final；格式及diff通过。账本1092exact/191parameterized、固定BASE、完整18任务和Task6 FIX_REQUIRED未改变。

## 元数据完整/擦除案例通过与消费者回归（2026-10-06）

同一47496终态exit0，task6-metadata-erased-and-full-regression.log：2PASS/0FAIL/0ignored/87filtered。完整固定BASE原观察与生产Arc<dyn ResultSetMetaData>入口的实际对象/具体后端身份/Debug均执行通过。现有验证器重新推导固定BASE，并验证完整目标观察数量门禁：SELECTED_METADATA_BASE_SOURCE_FLOOR=1/1；原assert17/assert_eq18/assert_ne2/matches1，新目标相同原断言加matches1（总matches2），调用数量门禁通过。不修改BASE或验证器规则；此为单案例源门禁，不是完整C2。

整个RDBC第一次回归终态exit101，task6-metadata-rdbc-regression.log，MockResultSetMetaData缺Debug；为原所有权fixture真实实现derive(Debug)，未改变生命周期/actor计数断言。第二次同一49511终态exit101，task6-metadata-rdbc-regression-ownership-debug.log，StrictResultSetMetaData因缺Debug产生22编译诊断。修改生成器及受控common输出，保留原精确trace及错误断言；写入前核对common完全匹配旧生成器，保护手工内容。生成check发现result_set.inc.rs仅_update->update一个既有签名漂移，确认仅变量名后同步，生成14文件一致。

第三次同一14574终态exit101，task6-metadata-rdbc-regression-trace-debug.log：16结果块/470PASS/1FAIL/0ignored，含1doc。唯一失败为standard_wrapper_contract_test的独立runtime消费者，ResultSetMetaDataObject缺Debug。为它真实实现Debug，并追加独立消费者的擦除后具体目标识别、标准对象地址、Debug、未知目标拒绝与独占可变标准视图验证，保留原17role/继承/NULL/错误/borrow检查。

最新完整RDBC回归同一54645 LIVE，task6-metadata-rdbc-regression-external-debug.log；终态前不宣称471/0或外部runtime通过。新增Debug要求的消费者影响已实际证实，后续Druid全lib/integration/外部旧新路径及API/SemVer门禁仍须验证。元数据迁移账本尚未更新，1092exact/191parameterized/固定1283不变；Task6 FIX_REQUIRED与完整18任务范围保留。

最新RDBC完整回归54645终态exit0，task6-metadata-rdbc-regression-external-debug.log：16结果块/471PASS/0FAIL/0ignored，含1doc。独立runtime消费者新增的erased具体目标/Debug/原对象地址/独占可变标准视图实际通过，原Wrapper所有17role/继承/未知目标/NULL及独占借用拒绝、dyn downcast拒绝案例均保留并通过。所有权fixture及435方法行为测试保持原断言；生成14文件check通过。此前470/1与两次Debug编译失败均保留作接口影响证据，不覆盖原日志。

Druid全部lib与standard_metadata_ownership_test、task6_rdbc_external_consumer_test_parity、buffered和Filter矩阵已启动同一11292，日志task6-metadata-druid-consumer-regression.log；未经终态不宣称Druid全量或外部旧新路径通过。完整BASE/sourcefloor1/1与private/erased2/2均已有实际证据，账本仍暂不绑定，等待相关消费者回归。Task6 FIX_REQUIRED/完整C1/C2/API/SemVer/MSRV/features/审查及完整18任务范围不变。
