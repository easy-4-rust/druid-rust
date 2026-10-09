# Task6 方法证据重新绑定前的覆盖保留验证（2026-10-07）

Task6仍FIX_REQUIRED；正式479行方法账本尚未修改，完整18顶层任务及Task7八义务保持OPEN。

## 实际RED与修复

新增两个回归控制：C2主证据重新绑定后原独立default执行测试必须保留且不重复；真实libtest参数化stdout不能打断严格成功函数标记。首次20项/2ERROR中一项是夹具签名未使用账本规范化空白；仅修正夹具后再次实际RED为20项/1FAIL/1ERROR，分别准确暴露原补充default测试丢失与成功函数未被证明。两个RED日志均保留，不把夹具错误当产品行为RED。

生成器显式保留PhysicalConnection::execute的原DEFAULT_UNSUPPORTED补充控制，主证据仍必须来自C2 receiver-exact原始调用，未放宽方法验证器。C1运行方式从nocapture改为show-output，严格成功函数和完整案例digest解析均保持。GREEN全部20项通过。日志task6-method-evidence-preservation-green-2026-10-07.log。

## 原生成测试字节证据

以唯一重新绑定候选运行原生成器--matrix候选--check，实际exit0：PASS generated method behavior matrix: files=14。全部14个生成文件与当前已登记文件逐字节一致，没有写入、删除或缩减Rust测试。独立回归还确认补充身份只出现一次，无关source对象不被纳入。

日志task6-method-candidate-generated-check-2026-10-07.log；候选仅改PhysicalConnection::execute的behavior_evidence，BASE签名、目标、转换及全部其他字段保持。候选SHA9f266d05760de5e0044b9eec16c9fca74d446031bbe13e72bb84a5b551a9c1ce，正式SHA7a96fdbec7484fed1461a756422ef9b05d25a6f8b1dd10ce305e353e643787d3。

## 完整验证状态

完整候选479项静态验证session12622已启动，日志task6-method-candidate-static-2026-10-07.log，尚未终态；不能作为运行验收。完整C2 session33663继续原执行句柄，已确认其真实Cargo子进程执行all-features完整目标，仍未整体终态。运行期间不修改Rust/Cargo/proto/SQL输入。

实际C1/C2终态、API/compiler/SemVer/MSRV/features/最终审查仍待完成。工具安装方案已提交用户确认，授权未到达前不安装；无分支/stage/commit/push/发布/系统安全配置变更。

## 后续实际终态与私有测试路由

session12622已终态exit1，首败PhysicalResultSet::date的目标签名漂移；上述LIVE描述由此替代。全部479当前目标签名独立审计发现六项，均为参数名：date/time/timestamp的_calendar→calendar，object_as的_column/_target_type去前缀，update_value的_update去前缀，update_value_by_label的column_label增加前缀。类型、接收者、同步/异步、返回值、目标路径和UFCS均不变。初始候选生成保护仅允许_calendar变化，因此实际拒绝另外三项并保留审计，不自动接受其他变化；核对当前真实源码后，以六项显式重命名列表生成第二候选。

新候选SHA65ae308dfa52b6bc0b4548ade2b3834fd828495e52dd34e01d598078e3608abd。仅候选的六项target_signature/hash和原execute主证据变化，所有BASE/转换等字段逐项保留；再次renderer --check实际exit0，全部14测试文件字节一致。正式479矩阵尚未修改。完整新候选静态session97260仍LIVE，日志task6-method-signature-candidate-static-2026-10-07.log。

C1运行器之前只支持integration目标，对已识别的私有lib来源仍报unknown package。新增两包私有测试路由控制实际RED21项/2ERROR；复用C2显式LIB_TEST_TARGETS，运行完整--lib并要求完整模块名精确匹配后GREEN21/0，未登记来源/模块、同名leaf和前缀伪造均拒绝。报告统计原把模块名中的双冒号计为parameterized；独立真实RED22项/1FAIL，改按parsed case_id计数后GREEN22/0。没有改变方法源合同检查、C2来源要求或运行断言。尚未有真实完整C1运行PASS。相关C2与删除门禁自测在修正PYTHONPATH调用后27/0；最初import失败日志保留，不算产品测试RED。

## C2 缓存缺失终态与完整重跑

session33663已终态exit1，druid-wrapper/__lib__因offline缺失锁定依赖arrow58.4.0在运行前失败；1090输入全部SHA一致。日志SHA42ec9b2f8c8f2af79205d183112d3d48b7c0497fa533642daa8a8d735eaf0f02，summary记录aggregate_test_counts=null，不猜测隐藏成功目标数。

按既有Cargo.lock执行fetch --locked --target x86_64-pc-windows-msvc，实际exit0，补齐缓存中的Arrow等既定依赖；没有升级依赖、安装CLI或改变离线验收命令。日志task6-all-features-locked-cache-fetch-2026-10-07.log。随后完整C2默认执行runtime3已启动session13316，1090构建输入重新冻结，仍使用1.97.1/all-features/locked/offline/-j1/test-threads1。保持完整1283分母，无整体PASS声明。

工具安装授权仍待答复；历史workspace3795/11、Admin49/11及所有18顶层任务继续OPEN。

## 全部479静态终态与正式绑定

session97260实际终态exit0，TASK6_METHOD_ROWS=479；INTERNALIZED=2、MERGED_EXACT=215、MERGED_TYPED=262。C2来源行43、fallback436、unique selectors442、parameterized rows33；明确TASK6_METHOD_EVIDENCE_EXECUTED=no。日志SHA668f0ad0eff0fa893c084be159d4899294e36ec3385281709f3c55d0dc624eb0，方法矩阵SHA65ae308dfa52b6bc0b4548ade2b3834fd828495e52dd34e01d598078e3608abd。

在取得真实exit0后执行保护式绑定：核对正式旧SHA、候选SHA、C2反向索引SHA、静态证明、14生成文件证明、1090冻结构建输入以及全部479逐字段差异。只允许execute的behavior_evidence，以及六个明确目标的target_signature/hash；其他字段均保持。旧正式字节保存于drafts/task6-method-matrix-before-validated-binding-20261007.tsv。实际绑定exit0，summary状态BOUND_SOURCE_VALIDATED_RUNTIME_PENDING，日志task6-method-validated-binding-2026-10-07.log。

绑定后原生成器--check再次实际exit0，14个Rust测试文件全部一致，没有更改构建输入。此前“正式矩阵尚未修改”由本节新状态替代。完整C2仍为原session13316真实LIVE；为了不并行启动第二组Cargo构建，完整实际C1须在当前C2取得终态后执行，不能用静态PASS代替。Task6、完整18顶层、Task7八义务、全部API/MSRV/feature/网络/审查门禁仍开放。
