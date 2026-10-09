# Task6 真实 SQLite 私有结果状态合同（2026-10-05）

## 范围与事实源

继续完整 P04 Task6–8、P05–P07；Task6 保持 FIX_REQUIRED。上一轮真实 SQLite owner/property 恢复及fetch hint修复有全量3662PASS/2FAIL终态，不是无进展。本轮补齐原关闭计数、游标与抓取计数及私有状态的真实证据，沿用已存在的 crate-private lib-test 迁移方式。

BASE：`5afe860:crates/druid/tests/druid_pooled_result_set_differential_test.rs`，七个原函数分别对应关闭计数、构造耗时、游标索引、抓取峰值、内部原始游标、属性完整读写/清空、非零身份。原建表/插入五行/SELECT * FROM t 保持。

## 实现与观测范围

新增 `crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs`，仅由cfg(test)子模块编译。真实 Toasty SQLite Connection创建原数据及驱动Statement/ResultSet，由实际 DruidPooledStatement、代际准入、资源注册器与 DruidPooledResultSet::new_standard 接管。测试直接读取生产代理共享状态和Filter context；不把测试计数当作真实close_count，不使用静态检查或汇总Stats替代，也不新建公共驱动接口。

关闭计数精确验证0->1；重复ResultSet关闭、创建者关闭和registry close_all之后仍为1。cursor_index和fetch_row_count各保留0->1->2。属性保留原零大小、空map、missing、put42、大小1、clear后0全部断言，并补充实际值读取。内部raw游标在close前真实开放、close后被释放。构造耗时BASE只读取、不保证Some，原边界保留。

测试构造覆盖真实驱动和生产代理/注册器；它不新增整个连接池借出、归还、取消或全部Filter的认证声明。

## 迁移绑定与验证器

七行原BASE元数据及SHA不变，disposition从参数化目录改为MIGRATED_EXACT，并绑定完整 `core::druid_pooled_result_set::diagnostic_tests::...` libtest身份。对应七个通用矩阵case id移除，避免重复计入或继续由fallback冒充。总分母1283保持，目录现1072 exact/211 parameterized；这些目录数量不是整体完成率。

新门禁永久自测先验证未登记私有路径被拒绝：task6-result-set-diagnostics-verifier-binding-red.log ERROR1/1；加入七个精确白名单函数映射后全15/15PASS。之前使用-m unittest导致import失败单独保留task6-result-set-diagnostics-verifier-red.log，不当作绑定缺陷RED。未开放全部src目录，未知私有target仍拒绝；Cargo运行证据依旧要求实际成功测试名称。生产行为无RED/修复声明。

迁移授权首次正确报告派生文件级表漂移；按既有renderer更新生成区，保留人工行为章节，随后authority79/pending25/physical73和原SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc通过。函数ledger、方法反向索引、生成文档保持同一事实源。

## 实际终态

- task6-result-set-diagnostics-first.log：3/3PASS。
- task6-result-set-diagnostics-final.log：7/7PASS。
- task6-result-set-diagnostics-lib-matrix.log：session64735 exit0，Druid lib66/66 + 原ResultSet matrix4/4 =70/70PASS。
- 迁移门禁全自测15/15PASS；最终purity43、authority79/pending25/physical73、三Rust文件fmt check和git diff --check通过。
- C2 --no-execute：session26016 exit1，task6-result-set-diagnostics-c2-final.log推进到 hidden_columns_none_by_default 缺少显式分支。此前三项版本C2失败construct_elapsed的log保留，已被七项绑定取代。整体C2未完成，后续fallback不认证。

本轮只改变测试、cfg(test)模块声明与派生迁移记录；没有新生产行为。上一轮默认workspace3662PASS/2FAIL属于新增七项测试之前的源，不能冒充当前全量回归。最新范围是70/70。Callable原prepared5vs6及typedindex/cache、普通Prepared创建后拒绝、精确调用形式证据仍开放；隐藏列同一游标设置/清除、完整映射/默认/资源/强类型合同仍需恢复。

完整C1/C2/API/compiler/SemVer/features/MSRV/review、Task7八组件、Task8、P05–P07范围保留。所有本轮启动的进程/会话终态，没有提交、推送、发布、安装或分支操作。
