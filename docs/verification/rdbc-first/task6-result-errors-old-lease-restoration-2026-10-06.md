# Task6 结果集错误、语句计数与旧租约完整恢复（2026-10-06）

固定BASE案例 result_set_errors_typed_getters_and_old_lease_are_observable，body SHA aa5928ea68326f524fcea1af685a0f7556fe7d8119ab59eab3c6d6befb5f0a44。

## 原行为与实际失败

原RowSetResultSet.value在游标未定位时返回Other及精确文本result set cursor is not positioned on a row。真实Toasty SQLite工厂、实际创建语句、Custom目标错误优先级、三次实际语句异常、真实归还回调及同一底层连接在id19新租约复用均保留。新增底层数据指针身份及新租约SELECT1可执行观察；旧结果集仍ConnectionDiscarded，不用模拟计数器代替。

诊断40084终态exit101/0PASS1FAIL，实际InvalidArgument(result set has no current row)。最小缓冲适配器修复后54161终态exit101/0PASS1FAIL，前三个原错误观察通过，actual statement.exception_count=2而原要求3。日志task6-result-errors-old-lease-diagnostic.log及task6-result-errors-priority-recheck.log保留。

StandardAdapterResultSet保留原游标错误；DruidPooledResultSet.object(index)在既有底层分类后向实际creatingStatement记录一次错误，不重复fatal分类、不改变成功计数。正式lib完整案例经过类型错误/计数两个真实失败后通过，归还及旧租约观察已实际执行。

## 正式与绑定证据

48494终态exit0：lib94+FilterStat21+buffered matrix6=121PASS/0FAIL/0ignored。日志task6-result-errors-old-lease-formal.log。完整原观察floor3940exit0。

GREEN后仅该账本行最后四字段改为MIGRATED_EXACT，增加一个准确lib函数目标，移除旧矩阵对应无专属行为ID，更新原反向索引及79行派生文档。全部1283行前七字段由全量审计逐行核对不变。绑定后正式矩阵exit0/6PASS，不与121重叠重复计唯一案例。

全量源观察66825终态exit0：1283基线/账本，1098exact/185parameterized，1235PASS/48FAIL。JSON task6-result-errors-source-floor-audit-2026-10-06.json。完整C2 6942终态exit1，下一缺口result_set_metadata_preserves_complete_rdbc_column_contract。该检查仍不证明语义等价。

自测15/15，final纯度44，authority79/pending25/Physical73固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc，生成14文件检查、四文件rustfmt及diff检查通过。

## 当前全范围边界

本轮生产修改后的全仓85349正在运行，日志task6-result-errors-workspace.log；必须取得终态，不能用旧3710PASS/2FAIL覆盖当前源码。此次121/0只属于相关回归。Callable真实第六次创建、完整元数据缺失枚举查询、C1/C2/API/SemVer/features/MSRV/review仍开放；Task6 FIX_REQUIRED，Task7八组件/Task8/P05-P07共18顶层任务未闭环。未创建或切换分支、提交、推送、发布、安装。
