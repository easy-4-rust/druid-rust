# Task 6 对象身份与警告/元数据完整恢复

## 已验证的原合同

固定 BASE `5afe8609b177544ff7d18461aa84f69efe8ee90e` 的两个完整案例现在
进入 `crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs`，对应函数为：

- `task6_rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract`。
- `task6_sql_warning_and_result_set_metadata_preserve_structured_fields`。

对象案例保留厂商类型名、正确/错误 downcast、原底层对象类型、Debug、同一
Arc 身份相等与不同 Arc 身份不等、全部 10 个对象显示值与 SQL NULL 判断。
唯一 `physical()` → `value()` 重命名沿用既有公开合同 allowlist 中的批准处置。
既有 VendorObjectProbe 真实实现复用，没有恢复 Physical 类型树或添加假身份。

警告案例保留 first/state01000/vendor7 与 next second/vendor8。
两列缓冲元数据保留原标签、Boolean/Unknown 类型、false/true 可空性和
下标 0/宽度外 3 的 InvalidArgument。0 在标准类型化入口由 ColumnIndex 拒绝；
3 实际进入 metadata getter 后由宽度校验拒绝。没有用构造器检查替代两个观察。

## 正式执行证据

在全仓 session15535 终态后才接入测试，避免污染运行中的源码输入。
此前 standalone 各1/1只作预备证据，没有代替正式 Cargo 验收。

会话1309终态exit0，日志
`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-object-warning-formal-regression.log`。

实际命令：

```text
cargo test -p druid --lib --test task6_buffered_result_set_canonical_matrix_test --test standard_filter_stat_semantics_test --locked --offline -j1 --no-fail-fast -- --test-threads=1
```

实际结果为 lib93、FilterStat21、缓冲矩阵6，共120PASS/0FAIL/0ignored。
两个完整新案例均以完整正式函数身份执行通过。本轮没有生产源码修改，也没有
故意破坏已有正确行为来制造运行时 RED；此前 C2 缺明确分支是迁移验收失败。

## 准确绑定与源观察审计

只有正式120/0及相关回归终态后，两个原账本行最后四字段改为 MIGRATED_EXACT。
仅增加两个完整函数到现有精确 lib 测试名单；全部1283原行前七字段保持不变。
从旧缓冲矩阵移除这两个无专属行为分支的 ID，更新原反向调用索引和79行派生文档。

第一次绑定脚本因预设状态名与实际 MERGED_PARAMETERIZED(case_id) 不同而断言失败；
失败发生在账本写入之前。按实际格式修正后执行成功，没有放宽验证器。

会话29175在矩阵 ID 变动后重新执行该正式矩阵，终态exit0/6PASS/0FAIL，日志
`task6-object-warning-binding-matrix.log`。这个6/6与前述120/0存在重叠，不相加为唯一案例数。

全1283审计会话40618终态exit0：1234源观察PASS/49FAIL，账本1097exact/186parameterized。
JSON `task6-object-warning-source-floor-audit-2026-10-06.json`保留全部缺口原因。
统计仍不等于语义等价或整体完成率。

完整C2会话5925终态exit1，下一完整原案例
`result_set_errors_typed_getters_and_old_lease_are_observable`缺明确执行分支。
验证器自测15/15、final纯度44、权威表79/pending25/Physical73固定SHA、
方法生成14文件检查、两文件rustfmt及diff检查均通过。

## 全仓与未完成边界

此前完整全仓15535终态exit101/242结果块/3710PASS/2FAIL/0ignored，含2doctests；
唯一失败目标仍是 Callable 原完整合同prepared实际5/预期6。Admin gRPC46/46PASS8.25s。
该全仓结果覆盖标量生产修改，早于本轮两个测试新增；本轮新测试证据为120/0与
绑定后矩阵6/0，不声称另一次完整全仓通过。

完整元数据合同仍有实际编译9处E0599，报告
`task6-complete-metadata-contract-audit-2026-10-06.md`；纯值枚举查询缺口尚未修复。
Task6仍FIX_REQUIRED，Task7八组件/Task8/P05–P07全部18顶层任务保持未闭环。
未切换分支、暂存、提交、推送、发布或安装工具。
