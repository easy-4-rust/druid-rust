# Task 6：ResultSet 更新重载与列名身份恢复（2026-10-05）

沿用 ADR-RDBC-006 和 single-standard-unification 计划。固定 BASE 为 5afe8609b177544ff7d18461aa84f69efe8ee90e，原案例 all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity 来自 druid_pooled_result_set_semantics_test.rs。

## 完整合同与实际实现

恢复全部 56 次原调用：标量 index/label、可空值、BigDecimal、日期时间、厂商对象、scale-or-length、NString，以及 ASCII/Binary/Character/NCharacter 的 unspecified/int/long 长度。保留原参数、负长度、厂商对象 downcast、每次完整描述符比较、最终56次调用和输入流/Reader未关闭断言。旧重载以已批准的 canonical ResultSetUpdate typed descriptor 表达，不重新引入 Physical 端口。

夹具使用真实 Toasty SQLite Statement、标准 registry/admission/policy，再按原 BASE wrap-probe 场景由实际 DruidPooledResultSet::new_standard 接管标准驱动探针。探针记录真正收到的下标或列名及描述符；流与厂商对象的 PartialEq 是共享句柄指针身份比较。没有预填调用、伪增计数或用通用 BufferedRowSet 更新代替原观察。

生产 DruidPooledResultSet 补充现有 ResultSet::update_value_by_label 的覆盖实现。普通列名直接转发驱动列名入口；配置列治理时保留隐藏/映射校验。与下标更新使用同样的开放检查、资源锁、错误分类和结果集错误统计。补充动态 Box<dyn ResultSet>、隐藏列、映射缺失、完整正反映射、关闭后拒绝/无额外驱动调用测试。

## TDD 与验证

日志目录：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

- task6-update-identity-red.log：测试夹具初次编译失败，缺 next 和 OpaqueObjectValue 实现；单独记录，不当作行为 RED。
- task6-update-identity-runtime-red.log：退出101，0PASS/1FAIL，null_value 列名被错误解析为下标，尚未抵达驱动列名入口。
- task6-update-identity-green.log：退出0，完整原案例1/1PASS。
- task6-update-identity-regression.log：退出0，6结果块，124PASS/0FAIL/0ignored。Druid lib77、FilterStat21、metadata ownership3、buffered matrix6、Filter coverage16、Filter canonical matrix1。
- task6-update-identity-c2.log：退出1，下一处 all_strong_getter_overloads_preserve_raw_spi_argument_identity 缺明确分支。整体C2仍FAIL。
- verifier自测15/15PASS；final purity43文件PASS；authority79行、Task7pending25、Physical disposition73，固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc不变；6文件rustfmt --check及git diff --check通过。

## 迁移与剩余范围

单个原案例绑定明确私有 lib 测试，移除原矩阵泛化占位；新增白名单只涵盖该准确函数。原1283分母与BASE摘要不变，当前1081exact/202parameterized。反向索引及派生迁移文档已同步。当前至少63项缺明确分支：buffered25、lease2、recycle12、ResultSetFilter24；已存在分支仍需完整行为验收。

本次没有新全Workspace回归；旧metadata版3677PASS/2FAIL早于字符串、常量及本次修复。最新证据仅覆盖上述124项。既有Callable原合同prepared5vs6、C1/C2完整证据、API/SemVer/all-features/MSRV及最终审查仍未关闭。

Task6维持FIX_REQUIRED；完整目标仍是Task6–8与P05–P07。下一步恢复完整强类型getter重载合同，不缩减原始断言、参数或驱动调用身份。所有本轮运行句柄已终态，未提交、推送、发布、安装或切换分支。
