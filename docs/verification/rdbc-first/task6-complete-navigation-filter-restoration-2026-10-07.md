# Task6 完整导航与属性 Filter 合同恢复（2026-10-07）

完整18顶层任务保持开放，Task6仍FIX_REQUIRED；本报告不关闭Task7八项义务或发布验收。

## 固定BASE合同与修复

原案例 `result_set_filter_semantics_test.rs#pooled_result_set_routes_navigation_properties_and_real_sqlite_state` 的26次短路观察完整保留，并补充完整有序事件列表。保留原行99/列88/方向2000/类型2004/并发2007/holdability2、absolute50/relative-20、精确filtered findColumn failure、实际创建Statement异常1；相同工厂的第二个真实SQLite两行状态机、取值10、前后位置及关闭后状态全部保留。

公开标准Filter提供惰性navigation future及属性request/value hook。私有链按注册顺序进入，默认委托唯一标准驱动接口，短路不轮询下游、不读取原生行号或伪造抓取峰值。属性解码在同一策略分类闭包内完成，不兼容返回类型明确InvalidArgument；失败只向原创建语句计数一次。wasNull默认仍保留原before/after链。Next继续原既有next Filter链；新的完整navigation hook包围其他导航。is_closed保留已关闭或失效租约时的本地状态快照，不访问驱动或Filter；其他本轮操作先拒绝关闭/旧租约。

ResultSetType、ResultSetConcurrency及FetchDirection沿既有holdability模式增加DriverValue，保存原已类型化整数；FetchDirection::try_from仍拒绝2000等未知输入。缓存键新增实际RED暴露MIN被折叠为0，改用真实rdbc_code保存类型和并发身份，原标准常量不变。公共API/compiler/SemVer/external-consumer完整门禁仍开放。

## 实际测试与错误记录

- 40717：初始接口编译失败，日志task6-complete-navigation-filter-interface-red-2026-10-07.log；不能当运行RED。
- 54778：完整lib运行RED，126PASS1FAIL，Previous短路原断言失败。
- 84572：路由首次编译失败E0283，递归future的Send推导未收敛；改为显式boxed future。
- 24711：完整lib127PASS0FAIL，原导航合同首次GREEN。
- 补充控制首次编译失败E0433，测试引用未依赖futures；改用标准库poll_fn，无新增依赖。
- 9554：真实补充RED，129PASS1FAIL，仅缓存键MIN->0失败；导航顺序、短路、取消与治理控制通过。
- 1689：四目标154PASS1FAIL；lib130、FilterStat21、MetadataOwner3通过，外部夹具误假设Toasty支持set_fetch_direction。保留真实UnsupportedOperation及原Reverse参数，不增加或伪造驱动能力。
- 最终四目标155PASS0FAIL，默认Rust1.97.1。真实DruidPool外部Filter验证公开hook、ABC顺序、短路后原值20、默认原行2、setter参数、精确错误、错误类型、关闭/旧租约。
- 61573：正式绑定后的10目标块168PASS0FAIL0ignored，默认Rust1.97.1；包含完整lib130、外部导航1、FilterStat21、MetadataOwner3、完整RowSet导航3、外部Metadata1、缓存LRU5、保留矩阵1、外部Statement1、外部读取2。重复的前轮范围不累加。

各日志位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`，终态summary保存结果、范围及日志SHA。局部测试不是完整workspace、all-features或MSRV1.95证据。既有未使用代码/变量警告仍存在，没有宣称全仓无警告。

## 正式账本与剩余门禁

仅原导航完整case改为EXACT，仅移除旧矩阵对应占位ID，并窄注册真实lib目标。所有1283前七冻结字段逐项保持；正式1130EXACT/153PARAMETERIZED。完整源码观察审计1267PASS16FAIL，不等价于运行通过率。

43005：完整C2 --no-execute终态exit1，下一完整 `pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors` 缺显式分支，整体仍FAIL。验证器17自测、final标准纯净性48、authority79/pending25/physical73及固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79通过。

新鲜全workspace默认1.97.1回归已启动session23625，日志task6-complete-navigation-filter-workspace-2026-10-07.log。启动时1023项Rust/Cargo/proto/SQL输入SHA已冻结；当前尚无终态，不写全仓PASS，后续必须检查同一句柄及冻结输入。Rust源及Cargo输入在运行期间保持不变。此前3787/0与默认all-targets check属于较早源码。

Task6完整剩余合同16、Task7八义务、Task8/P05/P06/P07、API/compiler/SemVer/MSRV/features/当前全仓/最终审查及全部18顶层任务继续开放。下一步准备完整Object更新原断言、重载身份、真实SQLite能力错误和实际异常计数，等待当前全仓终态再接入Rust源。

最终格式及Git diff检查实际exit0；1023项workspace冻结输入复核全部未变。下一完整Object更新原文与单对象原夹具已保存，manifest记录SHA和原断言分布，仅准备态，NOT_COMPILED/NOT_RUN/NOT_BOUND。
