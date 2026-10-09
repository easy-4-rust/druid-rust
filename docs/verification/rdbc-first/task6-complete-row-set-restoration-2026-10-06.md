# 完整 RowSet 转换与共享句柄恢复 — 2026-10-06

## 上轮恢复记录（以下纯度失败已在本轮解决）

上一轮全仓78227已终态exit101：243结果块/3716PASS/2FAIL/0ignored含文档测试。仅Callable canonical目标两项实际创建5vs6失败。日志task6-complete-metadata-workspace.log属于新增RowSet工作之前的源码，不能作为当前新增源码的全仓证据。

完整正式测试task6_complete_row_set_contract_test.rs保留固定BASE14列、三个原成功/失败/stream helpers、导航、真实底层指针、克隆相等/不同后端不等、Debug重命名和关闭状态观察。BASE body SHA5b94c94b4a76961891c4c03b4c33054787802cb608bc6cf8e6c96e015d7bb8f7。另有真实共享游标/借用冲突和真实后端关闭失败/重试/重复转发两项补充。

正式首次编译exit101/E0432缺失SharedResultSet，是接口缺失证据，不是运行时RED。首次实现Debug错误要求dyn ResultSet支持Debug，编译E0277，已改为打印真实底层指针。正式运行exit101/2PASS1FAIL，完整case在Bool->String失败；独立诊断另外实际0PASS3FAIL确认Bool->Long、Bool->Double、Int->Bytes失败。正式完整case修复后exit0/3PASS；buffered matrix6、metadata2、typed defaults9合计20PASS/0FAIL。首次GREEN命令使用错误测试目标名，单独保留task6-complete-row-set-target-name-error.log，纠正后实际执行结果才计入。

## 实现与尚未通过的边界

BufferedRowSet覆盖自身string_value/read_object，复用现有default_object_as单次标量转换；Long/Int/Short/Byte请求先保留i64后由标准扩展检查宽度，Double/Float先保留f64再执行原narrowing。其他请求复用从旧默认方法逐字提取的crate-private default_read_object，保留严格默认读取、日期、scale、资源和错误优先级；未增加第二结果集trait。

共享句柄当前Arc<tokio::sync::Mutex<Box<dyn ResultSet>>>实现真实克隆身份，映射借用防止替换Box，关闭原样转发。然而标准纯度最终检查exit1：unknown path root in extensions/shared_result_set.rs: tokio。该架构门禁未通过，不能因功能GREEN宣布共享组件完成，也不得放宽检查。下一步改为标准层允许的运行时独立共享借用，保持Send异步关闭、等待借用、取消后真实后端可恢复，并重新验证全部合同。不能用同步阻塞锁跨await或替换为内容/虚构ID比较来绕过问题。

迁移权威79行/Task7 pending25/Physical73原SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc仍PASS。新增完整case尚未改绑定，源码观察1236PASS47FAIL/exact1099parameterized184暂保持；Task7八义务与完整18顶层任务仍未关闭。没有新全仓、API/SemVer/default compiler/MSRV/all-features验收完成声明。

## 本轮运行时独立借用恢复及实际验收

标准扩展实现已移除Tokio锁依赖。SharedResultSetState使用标准库短时状态锁，实际Box后端由SharedResultSetBorrow独占拥有；驱动调用与await期间不持锁。借用Drop归还同一个Box并唤醒等待者；等待future取消移除自身队列项，取消队首时唤醒下一项。关闭future和拥有式borrow均通过Send编译约束。不缓存原始关闭结果、不宣称驱动自身取消安全或Druid监督式owner释放义务完成。

新增关闭等待/取消和活跃关闭取消恢复两项在旧实现上5/5通过，属于重构前行为基线；旧实现纯度检查为真实RED。标准库实现纯度转为48文件PASS；新完整正式6项包括不启动Tokio的队首取消/真实Waker唤醒/排队公平合同全部PASS。最终绑定后Druid lib94+buffered6+metadata2+complete6+typed9=117PASS；RDBC相关四目标444PASS；Wrapper实际SQLite/error-priority3PASS。合计564PASS/0FAIL，日志task6-row-set-final-binding-green.log、task6-row-set-rdbc-related-green.log、task6-row-set-wrapper-related-green.log。

唯一完整案例在运行/purity通过后从参数化绑定改为MIGRATED_EXACT，原1283案例前七字段全部核对未变。账本1100exact/183parameterized，源码观察1237PASS/46FAIL。JSON为task6-complete-row-set-source-floor-audit-2026-10-06.json；完整C2 --no-execute仍exit1，下一项row_set_result_set_preserves_labels_navigation_and_validation。没有调整检查器、断言期望或案例分母。

79行权威表一次当前target改动被严格检查拒绝，已保留冻结交接target和Task7 pending25；notes准确记录实际SharedResultSet位置。79行派生文档重新生成并PASS，73行Physical输入未修改。当前权威SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3（notes改变），不是旧权威SHA。最终具体target reconciliation仍需Task7验收。

验证器自测15/15、生成方法证据14文件、标准纯度48及格式/diff检查PASS。Task6整体FIX_REQUIRED；Task7八义务、Task8、P05–P07合计18顶层任务未关闭。最新当前源码尚无新全仓；旧3716/2在新增SharedResultSet之前。API/SemVer、默认1.97.1/MSRV1.95、all-features、Java差分、多数据库与最终审查仍未验收。当前自己的Cargo/Python运行均终态，没有新全仓启动。下一项固定BASE导航case含非法999 fetch direction、非法0列边界与精确closed Other(result set is closed)合同，不能只映射成功导航。
