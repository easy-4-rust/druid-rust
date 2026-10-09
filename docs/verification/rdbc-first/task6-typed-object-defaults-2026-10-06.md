# Task 6：typed getObject 与列名流合同恢复（2026-10-06）

沿用 ADR-RDBC-006、现有 single-standard 规格与计划，固定 BASE 为 5afe8609b177544ff7d18461aa84f69efe8ee90e。完整目标仍是 P04 Tasks 6–8 与 P05–P07，共18个未关闭顶层任务。

## 最终行为

标准 ResultSet::object_as 恢复十三类标量转换和 SQL NULL；八类资源目标进入对应 ResultSetExt getter，保留 typed descriptor 与资源身份；Custom 先读取列一次，保留驱动前置错误，再返回准确 result_set_get_object_typed_custom。label 默认解析一次后进入 index getter。Decimal Float 使用 BASE from_f64 的二进制精度，拒绝 NaN/无穷；平台转换函数留在 types/sql_scalar 内并仅 pub(crate)，标准接口不直接依赖第三方数值 trait，没有新增公开类型。

DruidPooledResultSet 的 typed 读取失败经过既有 access_core/policy 分类，再在实际创建语句上累计一次异常；开放检查继续在外层。完整原真实 SQLite 表/插入/查询保留所有标量、日期纳秒、SQLite NUMERIC 舍入值、NULL、Byte 溢出和 Custom 错误，最终 exception_count==2 未弱化。

完整原 labeled 四列 BufferedRowSet 经真实 SQLite Statement、registry admission 和池化代理接管，保留 short/byte/float/bytes/n_string、binary/ascii/unicode stream 和 character/n_character Reader 全部原始断言。修复字符串流与 Reader 的 Value 展示引号泄漏，返回原内容。

## 验证证据

日志位于 .superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

- 前一轮全workspace session56913 终态 exit101 /241结果块 /3689 PASS /2 FAIL /0ignored，含2doctests；仅 task6_callable_canonical_matrix_test 两项失败。该结果早于本轮源码，不能作为当前全量结论。
- task6-typed-object-default-red.log：exit101 /1PASS /4FAIL，实际默认入口缺失、Custom 错误优先级和资源分派失败。
- task6-typed-object-decimal-red.log：session57564 exit101 /0PASS /1FAIL，0.1 被格式化为十进制短值，违反 BASE 精确浮点转换。
- task6-typed-object-default-green.log：session57267 exit0 /6PASS /0FAIL；包含完整三项原测试与三项补充。
- task6-typed-object-sqlite-red.log：session78969 exit101 /0PASS /1FAIL，仅最终创建语句异常计数0vs2；完整转换断言已通过。
- task6-typed-object-sqlite-green.log：session86199 exit0 /1PASS /0FAIL，完整原SQLite合同通过。
- task6-typed-object-labeled-base.log：session72345 exit101 /0PASS /1FAIL，ASCII stream 返回带引号字节；保留原期望，没有放宽。
- task6-typed-object-labeled-green.log：session52567 exit0 /1PASS /0FAIL，全部原列名重载断言通过。
- 最终 RDBC 包：task6-typed-object-labeled-rdbc-final.log，session75089顺序脚本第一步exit0 /16结果块 /471PASS /0FAIL /0ignored，含1doctest；435生成测试不是C1完整生产证据。
- 最终 Druid 回归：task6-typed-object-labeled-regression-final.log，同session75089终态exit0 /7结果块 /134PASS /0FAIL /0ignored；lib81、typed6、buffered6、FilterStat21、metadata3、FilterCoverage16、FilterMatrix1。
- task6-typed-object-c2-final.log：session38825 exit1，下一项 map_get_object_overloads_preserve_index_label_and_nullable_map_identity 缺明确分支；完整C2仍FAIL。
- 验证器自测15/15，final purity44文件、authority79行/Task7pending25/Physical disposition73及固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc通过；13个Rust文件 rustfmt --check 与 git diff --check通过。

纯度初次检查拒绝裸 matches!，随后拒绝标准层直接导入 bigdecimal。改为 std::matches! 和平台层真实转换函数后通过；验证器规则没有放宽。一次补丁上下文不匹配未写入任何文件，随后按当前源码修正。

## 台账与剩余范围

五项完整原测试改为准确可执行入口，BASE原1283分母、源身份、摘要及关键观察不变。三项为独立根integration测试，两项为准确私有lib入口；仅增两项私有函数白名单。删除五个旧矩阵phantom case ID，反向方法索引和派生文档同步。

当前1087 exact /196 parameterized；最低缺明确分支57：buffered19、lease2、recycle12、ResultSetFilter24。映射数量与分支存在不是行为完成率。

Task6仍FIX_REQUIRED。完整C1/C2、Callable原prepared5vs6、typed参数错误/owner/cache和call form身份、公共API/compiler/SemVer、all-features/MSRV及全范围审查未闭合；Task7八组件、Task8与P05–P07完整范围保留。旧文档与API快照不冒充fresh final认证。新全workspace回归 task6-typed-object-workspace.log 已启动，尚无终态，不在其编译过程中修改Rust。

下一项恢复原type-map getObject的下标、列名和Some/None map身份，不能通过无条件解析列名或普通Scalar拒绝替代。没有提交、推送、发布、安装或切换分支。

Fresh workspace session3877 confirmed LIVE by handle; log task6-typed-object-workspace.log. No terminal result claimed.
