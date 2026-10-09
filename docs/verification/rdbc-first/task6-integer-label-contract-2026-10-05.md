# Task6：整数读取与缓冲标签合同恢复（2026-10-05）

事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md`，Task6。
完整目标 P04 Task6–8、P05–P07 保持不变；本报告不声明完成。

## BASE 与实际 RED

5afe860 的完整 `int_getter_returns_integer` 使用真实 Toasty SQLite、原五行表、`SELECT id FROM t WHERE id = 3`；按下标取3，按标签要求准确 `InvalidArgument`，然后关闭资源。现已恢复完整 helper、明确 canonical arm 和独立运行入口。

第一次构建缺少 RdbcError 引用，保留 `task6-integer-label-red.log`，这是编译迁移问题而非运行时 RED。修正引用后实际 matrix 为 3PASS/2FAIL，两个入口均在原错误类型断言失败，日志 `task6-integer-label-runtime-red.log`。

原 `spi/rdbc_result_set.rs` 缓冲游标以实际 column_labels 做大小写无关的首匹配；缺失返回 InvalidArgument。Toasty 原实现用 `RowSetResultSet::new`，即没有实际标签。其他 DuckDB/HTTP/libSQL/JDBC Agent/RBDC 原实现用 with_column_labels；不能为了 Toasty 把所有共享适配器标签一律拒绝。原稀疏 trait 默认 UnsupportedOperation 仍应保留。

新增真实 SQLite 原始驱动测试 `buffered_column_label_contract_test` 在修复前 0PASS/2FAIL，日志 `task6-buffered-label-raw-red.log`：无真实标签查询错误类型错误、生成键真实标签缺失。

## 生产修复

StandardQueryResult 保存驱动提供的实际标签，StandardAdapterResultSet 接管同一标签；元数据合成 column_1 等占位名不参与查找。find_column 保留大小写无关、首匹配、1-based、unknown column InvalidArgument，以及关闭/租约失效 ConnectionDiscarded 优先级。

真实 SQLite 用例验证 SELECT3 按下标读取、不凭合成名提供标签、缺失标签准确错误、失效租约/关闭优先，以及真实 GENERATED_KEY 标签在空结果/有值3/大小写访问/关闭后的行为。未声明外部数据库或所有驱动家族已获得实时认证。

修正三处受影响的迁移测试：Filter coverage、ResultSet Filter canonical、共享映射诊断中的标签拒绝类型。保持精确 InvalidArgument（Filter update-by-label 保留 exact unknown column: label），没有改成宽泛 is_err，也没有修改稀疏标准 trait 的默认错误。

## 验证

`task6-integer-label-green.log`：24PASS/0FAIL，exit0，包括 pooled matrix5、Filter coverage16、Filter canonical1、raw label2。
final purity43 PASS；迁移权威79/Task7 pending25/Physical73 PASS，SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc 未变。
BASE1283与既有迁移登记保持不变；整数用例仍为原参数化行，现有明确行为分支执行完整 helper。不是分母缩减或通用 smoke 替代。

完整 C2与当前 default workspace 回归正在运行，分别为 task6-integer-label-c2.log 与 task6-integer-label-workspace.log；终态将在本报告追加。旧3673/2属于本次标签修复前源码。

## 未完成项

Task6继续FIX_REQUIRED：完整C1/C2、Callable prepare5vs6、typedindex owner/cache、普通Prepared拒绝、重载身份、API/compiler/features/MSRV/review均仍开放。Task7八组件、Task8/P05–P07未完成。后续从C2具体缺口继续，不把metadata proxy属性或字符串长度替换成公有metadata列数/通用统计。


Task6 integer-label C2 terminal 2026-10-05: session61277 exit1 advances meta_data_proxy_returns_proxy_with_attributes explicitarm, task6-integer-label-c2.log. Integer helper passed this static phase; whole C2 remainsFAIL. Supersedes LIVE61277. Next restore actual metadata proxy/sharedattribute contract through production getter, not an unrelated snapshot or fieldcounter. Workspace82988 remainsLIVE.


Task6 integer-label first fullworkspace terminal 2026-10-05: session82988 exit101 before any test result blocks; compiler reports time/moka requiredrlibformatmissing in two targets. This is not runtime regression evidence, no source-defect or concurrency-cause claim. No Cargo/rustc remained live at inspection; time/moka rlibs exist and E free114731798528 bytes. Safe retry is same locked/offline/no-fail-fast workspace with buildjobs1 and serial tests, confirmed LIVE59151/task6-integer-label-workspace-serial.log. Firstfailedlog retained; no clean/delete/install. Target24/24 remainsvalid, currentfullworkspace unverified. C2 alreadyterminalFAIL next actualmetadata proxy attributes. Fullscope/FIX_REQUIRED preserved.
