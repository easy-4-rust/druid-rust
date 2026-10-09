# Task 6：Callable 原始第六次创建与句柄拒绝源码核查

## 已核实事实

CodeGraph 当前源码显示 DruidPooledConnection::prepare_call 在 canonical factory 成功返回后才调用 cache_stats.record_prepare；factory Err 经过已有策略分类后直接返回，不记录成功创建。该行为不能为了原计数6而修改为“失败也计一次创建”。

当前完整原始错误案例的 ordinary-handle 测试工厂直接返回 UnsupportedOperation(connection_prepare_call)，未创建任何普通Prepared句柄。随后原 prepared_statement_count=6 与 closed_prepared_statement_count=6 断言保持不变。因此实际5不是补一个计数分支可以恢复的原行为；当前 fixture 未保留第六次真实创建、非Callable拒绝和原句柄关闭。

DruidPooledCallableStatement::new_standard 接收 Box<dyn CallableStatement>，当前在注册资源后捕获原始属性，并在属性捕获失败时分类错误和关闭已注册owner。源码未见另外的 raw advertised Callable Wrapper role 校验。Rust typed 返回值已限制接口类型，因此不能直接返回 Box<dyn PreparedStatement> 来冒充旧动态返回；这也不能自动证明原运行时非Callable拒绝已迁移。

“没有独立role校验”在此仅为源码事实，不是已经取得运行RED的产品缺陷。是否需要增加该门禁，必须由保留真实普通Prepared后端、类型/标准视图与实际关闭观察的完整原始控制证明；不得以假Callable空对象、提前Unsupported、构造失败或测试计数器替代。不能削弱原6/6断言或扩展错误白名单。

## 下一步和范围

当前workspace同一session41767确认LIVE，并已从编译进入真实测试执行，日志 task6-filter-update-workspace.log；不修改编译输入、不重启。前一workspace3704/2和最新相关134/0均不能代替其终态。

下一实现先取得当前回归终态，然后恢复完整原始控制与实际生命周期失败，再决定最小生产修正。typed index0构造拒绝仍不能证明原owner/cache错误路径。元数据完整草稿、C1/C2、54缺明确分支、Task7八组件、Task8/P05–P07及完整18顶层任务继续开放，无新增完成声明。


## 标准动态角色边界补充（2026-10-06）

本轮CodeGraph核实Wrapper::is_wrapper_for(Some(TypeId))使用canonical_unwrap(target).is_some()；StandardUnwrapped明确分别提供PreparedStatement与CallableStatement视图。该动态角色声明独立于Connection::prepare_call的Box<dyn CallableStatement>静态返回类型。当前ordinary-handle直接Err没有真实后端创建，仍不足以恢复BASE第六次创建/关闭。后续完整控制必须实际持有并转发普通Prepared后端的属性、执行与关闭，保留其真实标准Prepared身份且不广告Callable角色，经过现有生产创建计数和清理路径观察；任何类型投影必须只适配唯一标准接口，不能构造空Callable替身或直接假计数。是否需要生产角色门禁仍须完整运行RED决定。本段为源码边界证据和控制要求，尚未实现、编译或运行该控制，不关闭当前两个Callable失败。

同一workspace61393/cargo70220仍LIVE，本轮未改Rust/Cargo输入；完整42源码缺口、Task7八义务和18顶层任务继续开放。


## 同一完整回归等待期间的准入核对

2026-10-06继续CodeGraph核对prepare_call1291–1380及DruidPooledCallableStatement::new_standard298–388：底层factory成功返回后才record_prepare；new_standard先创建真实资源并登记registry，再捕获初始属性。目前没有显式动态Callable角色校验。当前ErrorCallableConnection的ordinary-handle直接Err(connection_prepare_call)，未创建或接管原普通Prepared句柄，因而不能验证原拒绝后的实际关闭、cache删除及prepared创建归属。真实完整案例仍精确要求attempt7/prepared6/cacheDelete6/closed6；不会改为5。

恢复控制时需要保存一个实际普通PreparedStatement的完整执行/属性/参数/close逻辑，其Wrapper只广告Prepared与继承Statement。若标准工厂静态返回Callable容器投影，该投影必须委托真实普通后端并明确拒绝OUT能力，不能用空Callable或失败factory替代真实句柄。先取得该真实控制的runtime RED，再确定准入校验的最小位置及登记后清理。检查须与已有registry取消安全、错误分类和真实cache所有权一起验证，不预先增加统计。记录型TrackingPreparedStatement有实际参数/执行/close逻辑，但是否适合原fixture仍需完整BASE核对；本轮尚未接入控制或修改生产准入。

完整workspace2850/cargo49196仍实际LIVE，Rust/Cargo输入保持；上述源事实不关闭两个Callable失败或18顶层任务。


## 真实普通后端投影准备（未接入 Cargo）

固定 BASE 的 ordinary-handle 明确返回 SqlTextPreparedStatement；原对象保存真实 SQL、属性和关闭状态，不声明 Callable 身份。原完整错误案例 body SHA 为 a26eb9e23fcf75bcf2e643117c98dcaeb8c8465c4c388968e6dc803f7e2a8f16。当前标准静态返回边界不能直接返回普通 Prepared，因此准备仅用于测试的 OrdinaryPreparedCallProjection：接管 TrackingConnection 实际创建的 Box<dyn PreparedStatement>，完整委托 37 个 Statement 和 14 个 PreparedStatement 方法，Wrapper 仅声明 Prepared/Statement，三个 OUT 核心入口明确拒绝。它不合成创建/关闭计数，不用空对象代替真实后端。

补充控制草稿验证真实角色、参数 17、query 值 22、query_timeout 9、实际参数记录、OUT 准确拒绝与重复关闭后底层 close_attempt/close 都为 1。该补充用于确认控制自身真实性；正式原案例仍须保留 attempt7/prepared6/cacheDelete6/closed6，并在投影接入真实 factory 后取得生产准入运行 RED。不能把补充普通执行控制通过当作原准入合同已恢复，也不要求原非 Callable 后端提供原合同未执行的 query 功能。

草稿存放 drafts/ordinary_prepared_call_projection.rs.txt 和 ordinary_prepared_call_projection_control_test.rs.txt。现有标准方法清单与草稿委托方法逐项审计 PASS（37+14），rustfmt 语法检查 PASS；未编译、未运行、未登记迁移绑定。SHA 和明确状态记录于 ordinary_prepared_call_projection_preparation_manifest.json。全仓回归同一2850仍在编译，真实cargo49196及变化中的rustc子进程已核实，不修改Rust/Cargo输入。当前1283正式floor1241/42和18顶层任务均保持。


### 完整原错误矩阵及底层关闭观察草稿

已准备 task6_callable_canonical_matrix_with_ordinary_backend.rs.txt，保留现有完整原错误函数 body 字节不变，未删参数调用/错误断言或降低 7/6/6/6。仅普通句柄 factory 分支改为真实 TrackingConnection::prepare_statement 后接管该后端，观察状态随同一 factory/connection 传递。新增独立池化准入控制要求精确 connection_prepare_call、actual prepare/cacheDelete/closed 各 1、底层 actual close_attempt/close 各 1，重复 connection.close/pool.close 后不增加；RED 阶段若误接受先关闭返回句柄，再保留失败断言。

该完整矩阵为非 Cargo 草稿，rustfmt PASS、原完整函数 body 比较 PASS；未编译、未运行、未绑定。清理路径源审计表明 new_standard 已先 registry 登记再读取属性，不能提前加 delete 计数凑原 6。后续必须用真实控制确定拒绝后的清理与统计。规格 Task 6 Callable 参数错误与缓存合同恢复已明确该完整义务，无需新建规格事实源。

格式化后的临时文本存在性检查首次因多行 ordinary.prepared_close_attempt_count 触发 AssertionError；未视为通过。已按空白无关方式检查该表达式，并重新通过原完整函数 body 字节比较。未修改正式验证器或生产输入。


### 拒绝后的缓存与监管清理边界

CodeGraph 源审计：Callable cache_lifecycle.rs:131–148 在实际 raw.close 成功后才记关闭；fresh cache-enabled 且 cacheable=false 使用 record_cache_delete（包含当前缓存数递减），cache-hit 使用 record_checked_out_cache_delete。直接在 factory 返回 Err 或伪造统计均不能恢复该行为。registry.close_registered_after_admission:308–338 先登记 cleanup queue/waiter、结束 admission，再由 coordinator 清理，调用者取消不能只靠直接 await raw.close 规避。后续准入修复需保留真实句柄、错误分类和上述所有权边界，并以正式运行控制验证；本段不宣称准入修复或 Task7 取消安全已完成。

同一全仓2850已完成27m57s编译并进入实际测试，真实cargo49196与测试子进程已核实。运行中已观察两项Callable原计数5vs6失败；尚非终态，不用运行中总数更新完成门禁。Rust/Cargo输入仍未修改。


实际完整准入RED：76432终态exit101，8例中5PASS3FAIL。普通投影真实性控制实际PASS（角色、参数17/query22/timeout9/底层close1）；原完整错误案例两个入口实际失败在wrong_handle精确拒绝，新独立池化准入控制失败于普通后端被接受。原7/6/6/6后续统计未执行，不提前称通过。日志task6-callable-real-ordinary-admission-red.log。随后仅在new_standard完成owner登记/装配后检查raw动态Callable角色，原拒绝经已有错误分类及registry监管清理，不增加假统计、第二驱动树或更改原断言。等待定向复测终态；全C1/C2/API/MSRV/features/review及18任务仍未关闭。


80952已终态exit101：8例6PASS2FAIL。新增真实普通后端准入拒绝控制实际PASS，包括actual prepare/cacheDelete/closed各1、底层close_attempt/close各1及重复关闭不增加；真实性控制继续PASS。原完整错误两个入口现在已通过精确wrong_handle/unsupported/attempt7/prepared6，失败推进至cached_prepared_statement_delete_count actual5/expected6（line650）；closed6位于之后尚未执行。不得将6/2写成全GREEN，原6/6统计未降低。源码进一步确认invalid_index的14个 index0观察都在CallableParameter::by_index(0)构造阶段返回Err，and_then池化方法没有执行；因此这些观察不能证明原owner/cache失效路径已迁移。后续须恢复原非法索引的真实池化执行边界及失效清理，不能通过set_poolable、额外不同参数失败或改统计预期替代原合同。全18任务和正式floor1241/42仍开放。


固定BASE再核对index_parameter：原实现先CallableParameter::by_index(parameter_index)，再record_result；该校验错误在当前owner内record_exception，未进入driver。因此前述“需要非法索引raw执行”是过强推断，实际应恢复owner内部校验/失效路径，不能引入非法ParameterIndex或额外不同参数错误。SPEC已补充同一原验收映射。14个原index0观察已改为当前invalid_index.index_parameter(0)后保留全部原and_then目标和assert；原7/6/6/6不变。新owner控制保留外部by_index0拒绝、有效index1原值7、owner0精确错误、关闭优先和实际cacheDelete/closed1。24621已终态exit101，17个E0599缺少index_parameter，实际compileRED。随后唯一标准Callable增加纯默认索引校验，Druid override先ensure_open、经已有classification退出缓存、真实statement异常记录一次。待运行复测；新公开方法的API/default/compiler门禁仍需补齐，不计Task6完成。


33336已终态exit0：完整Callable矩阵9PASS0FAIL0ignored。原两入口全部原14 index0错误、其余参数/OUT错误、普通真实句柄拒绝、attempt7/prepared6/cacheDelete6/closed6及原后续关闭均执行通过。独立owner校验也通过有效1/原值7、原0准确错误、closed优先及实际cacheDelete/closed1；普通真实Prepared关闭1控制继续通过。不将9局部通过当作全仓/C1/C2/公共API或Task6完成。新增独立标准默认control已接入RDBC，等待其运行；纯度48/final通过。生成证据--check曾失败，差异仅result_set.inc.rs的column_label同步为当前标准签名_column_label；最小同步后再检查，不修改生成器门槛或原case分母。


RDBC完整回归72416终态exit0，17结果块/472PASS0FAIL0ignored（含1doctest），新增独立默认索引控制实际PASS：0精确拒绝、有效1/usizeMAX严格selector、原OUT1及真实关闭。原435方法证据测试全部PASS；生成14文件--check在参数名最小同步后PASS。此结果不证明新方法已加入完整公共API/方法分母验收；该门禁继续待闭合。
