# Task6 池寿命测试确定性迁移与验证（2026-10-08）

## 变更及行为

上一轮正式Cargo产物的受控年龄诊断证明，原测试的首次get/close耗时超过10ms时，归还寿命检查会正常淘汰holder，idle=0；直接要求首次归还必为idle=1依赖调度。历史失败未记录年龄，因此不反推所有原失败的现场原因。

本轮将固定BASE的borrow_skips_lifetime_expired_idle身份迁入DruidPool私有单元测试。新夹具保持10ms寿命和min_idle=0，通过正式PoolInner::create_connection创建真实holder、记录空闲状态，保留idle=1和active=1计数断言；随后在测试内部直接将created_at设置为30ms前，验证借出得到不同holder、create=2/destroy=1、后续close/get/Drop和shutdown后物理关闭数等于创建数。

原来的首次get/close不再用作“必然在10ms内归还”的建模前提；该阶段被显式空闲夹具替代。另两项独立控制验证：10ms寿命下归还已知超龄holder必须active0/idle0/discard1/destroy1/close1/recycle0且物理关闭一次；3600秒独立正常控制验证同holder复用、只创建一次。未延长原超龄测试阈值，也未删除生产归还寿命检查。

只在druid_pool.rs新增cfg(test)私有模块声明，未增加公共测试接口或修改生产池逻辑。两个测试替身各自独立.rs，另一个.rs仅承载测试函数。旧Wrapper目标移除对应函数，仍保留其余九项。

## 迁移身份

原BASE: 5afe8609b177544ff7d18461aa84f69efe8ee90e。
原函数体SHA: 21c955beeda6cc5cb261ec20827aceb10ede3cdf04eae929f219eabfd8d62592。
新目标: crates/druid/src/pool/druid_pool/lifetime_tests.rs#pool::druid_pool::lifetime_tests::task6_pool_deep_branch_coverage_test_borrow_skips_lifetime_expired_idle。

候选与正式全量源码核对均PASS。1283行的前七个冻结来源字段逐项不变；仅一个目标路径/函数选择器变化，反向调用索引对应十一条来源调用记录重绑定。1146 EXACT/137 PARAMETERIZED、零移除保持不变。校验脚本仅新增精确LIB_TEST_TARGETS路由，没有改变来源分母、断言/调用密度规则或其他验证逻辑。

## 实际验证

所有Cargo使用显式Rust/Cargo1.97.1及独立target F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008。

| 范围 | 结果 |
| --- | --- |
| 私有寿命测试定向运行 | 3PASS/0FAIL，包含在下项，不重复累计 |
| cargo test -p druid --lib --all-features --locked --offline | exit0，153PASS/0FAIL/0ignored |
| Wrapper part_05目标，默认features，locked/offline | exit0，9PASS/0FAIL/0ignored |
| 迁移校验器自测 | exit0，18PASS |
| verify_task6_deleted_test_parity.py --no-execute | exit0，67文件/1283身份，1146exact/137parameterized，executed=no |
| 本次Rust文件rustfmt --check及定向git diff --check | exit0 |

初次rustfmt模块路径设置错误、首次Cargo编译漏导入Connection trait（3个E0599）均保留原日志，修正后实际运行通过；它们未进入行为测试，不作为产品行为RED。原调度依赖的行为RED见上一轮诊断报告。

## 证据与剩余范围

新快照task6-lifetime-formal-inputs1.json包含1107源码/58控制输入，终态变化0。相较上一1104快照，仅两个既有Rust文件变化、新增三个私有测试源；控制输入仅校验器路由及两份TSV变化。所有原文件前像备份保存在F盘；18份命令/log/摘要和新validation summary逐字节镜像到SDD目录，前缀task6-e-space-。validation summary还绑定实际153测试exe SHA。

本轮没有重跑完整C1/C2 runtime、Wrapper all-features完整范围或新鲜全workspace；旧1104输入3876PASS/44FAIL仅为迁移前最新完整结果，不充作1107输入终态。没有新的严格全workspace Clippy/API/MSRV/实库认证。当前测试迁移局部通过，Task6仍FIX_REQUIRED，Task7八义务/Task8 INCOMPLETE，18顶层任务OPEN。

后续应在1107输入上完成正式完整C1/C2和当前全仓验证，并继续资源标准唯一树与发布门禁。未创建分支、提交、推送、安装工具或修改防火墙。
