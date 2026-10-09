# Task 6 真实 SQLite 同步资源绑定保真

规格事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md` Task 6。
BASE：`5afe860:crates/druid-wrapper/tests/prepared_statement_semantics_test.rs`。

## 完整恢复范围

新增完整 `prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite` helper、显式 canonical arm、独立运行测试。真实 Toasty/SQLite 证明：短流/负长度 setter 报错、ASCII/Binary/Character/NCharacter/BlobStream/ClobReader/NClobReader/URL/RowId/null Blob 十参数落库；setter 后调用者读取剩余资源不影响执行；query/generic/batch 全路径与实际行值；Filter 三次执行快照及一次两批次快照保留原始类型/长度。

内部 common Prepared 增加默认关闭的 eager_resource_binding 策略，仅 Toasty SQLite prepare 启用同步流/Reader eager binding。原有 materializer 读取一次，将已物化值保存在 raw 参数槽，执行与 batch snapshot 不再次读取资源。异步 LOB 保持异步执行读取，SQLx 和直接 rich batch 保持原先延迟读取与取消/失败消费语义。无第二套公共 Prepared 接口。

## RED/GREEN 与测试纠正

- 实际产品 RED：目标10 PASS/2 FAIL，短流 setter 错误预期未成立。日志 task6-resource-binding-red.log。
- 修复后完整真实 SQLite 独立用例 PASS；首轮矩阵10 PASS/2 FAIL 位于旧48描述符测试，暴露了将记录型 driver 未消费资源断言错误推广到真实 Toasty 的测试缺陷。日志 task6-resource-binding-green-attempt.log。
- 48描述符用例恢复 BASE 记录型 driver，Tracking fixture 实际保存并可观察完整原始绑定描述符，独立比较 raw bindings 与 Filter snapshot；无真实数据库执行认证声称。之前以真实 Toasty/SQLx + Filter 拒绝证明全48绑定未消费的说明被本记录纠正。
- 原资源测试 blanket 延迟绑定预期分为两种明确驱动策略；Toast SQLite setter 消费/SQLx execution 消费都检查精确读取次数，清批次不新增读取、负长度不读取、短流按对应阶段报错、两行内容和关闭拒绝均保留。
- 最终目标矩阵12/12 + resource tests3/3 =15/15 PASS exit0，日志 task6-resource-binding-final.log。随后 Tracking fixture 清参同步清除绑定观察状态，该最终改动由完整workspace回归覆盖。
- C2 no-execute exit1，已推进到下一缺失 arm：result_set_statement_handle_reports_clear_batch_failure，日志 task6-resource-binding-c2.log。未降低断言门槛、未改1283分母。
- final purity43、authority79/pending25、7文件rustfmt检查均exit0；authority SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc。
- 完整默认 workspace locked/offline serial --no-fail-fast 回归终态：session44316，exit0，235结果块，3636 PASS /0 FAIL /0 ignored，含2doctests。日志task6-resource-binding-workspace.log。

Ruling: BASE 已规定具体 driver 同步资源消费，先前统一延迟策略和真实driver48绑定不消费假设错误；恢复driver原始行为，同时保留逻辑富描述符、SQLx延迟策略、直接rich batch以及异步LOB取消安全要求。这是批准的BASE保真目标的修复，不削减范围或重新暴露私有API。

Task6仍FIX_REQUIRED，C1/C2/API/compiler/SemVer/all-features/MSRV/fullreview与完整Task7-8/P05-P07保持未完成。
