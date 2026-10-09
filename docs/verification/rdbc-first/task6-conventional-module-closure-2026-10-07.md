# Task6 默认模块路径闭合（2026-10-07）

本轮完成剩余25处path属性清理及20个夹具目录迁移。只改变模块文件定位；模块名称、访问层次、测试函数、断言及夹具逻辑保持不变。Task6仍为FIX_REQUIRED，完整18顶层任务未关闭。

## 修改和保真证据

5处属性本来与Rust默认路径一致，直接移除。其他20处夹具移动到声明模块的默认子目录，原字节内容SHA-256逐项一致，目标路径全部验证位于仓库且原先不存在；没有覆盖文件或递归删除目录。

清单：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/module_path_closure_manifest_2026-10-07.json。

修改前源码闭合检查实际拒绝path属性；修改后调用同一检查器的完整verify_source_surface和verify_no_duplicate_trait_tree均PASS，扫描669源码文件，没有修改检查器规则。整个verify_no_physical_standard.py仍exit1：公共API生成要求nightly-2026-08-07-aarch64-apple-darwin，而当前Windows主机不可执行该cargo工具链。该门禁保持未完成，不能以源码子检查替代公共API/rustdoc验收。

## 实际运行与固定合同

- 双包完整lib session68468终态exit0：Druid105PASS、Wrapper10PASS，共115PASS/0FAIL/0ignored。
- 日志：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-conventional-module-closure-regression-2026-10-07.log。
- 与前轮日志逐名比较，115个实际执行测试名称多重集完全一致；证明未因目录迁移遗漏已有测试。清单为drafts/module_path_closure_runtime_roster_2026-10-07.json。
- 全部20个移动夹具内容哈希、8个声明文件的最终输入哈希在运行期间重新核对PASS。
- 新鲜完整源码观测审计：1283基线/1283账本；前七冻结字段全部一致；1259PASS/24FAIL，1122EXACT/161PARAMETERIZED。JSON为task6-conventional-module-source-floor-audit-2026-10-07.json，未覆盖历史审计。
- 权威表79、Task7pending25、Physical标准化73及原SHA保持PASS；标准纯度48文件PASS。
- 迁移验证器自测17PASS、权威验证器自测5PASS。
- stable、locked/offline、默认lib测试不代表MSRV/all-features/完整workspace通过。

## 保持开放

24个ResultSet Filter完整原合同尚待恢复；资源登记委托顺序检查和Wrapper跟踪调用点基线尚待核对。公共API/compiler identity/SemVer/MSRV/features、完整workspace新鲜回归、最终审查、Task7八类义务、Task8及P05–P07继续OPEN。不将模块闭合或115个库测试通过等同于整个计划完成。

新鲜完整C2源码检查：verify_task6_deleted_test_parity.py --no-execute，session6388终态exit1；仍首败pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads明确参数分支。该命令不运行Cargo测试；没有新增缺口或减少原24项失败，下一步恢复完整Filter合同。
