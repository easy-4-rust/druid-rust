# Task6 最终门禁执行与工具修复（2026-10-07）

Task6仍FIX_REQUIRED，完整18顶层和Task7八义务保持OPEN。本轮推进实际门禁与其缺陷修复，不修改产品Rust实现，不缩减1283测试或479方法分母。

## 删除门禁与跨平台夹具

删除门禁自测首次实际exit1：7项中1项ERROR，Windows Path.write_text将规范TSV的LF转换为CRLF。独立RED日志task6-no-physical-fixture-red-2026-10-07.log保留。夹具改为固定UTF-8字节，并新增真实CRLF仍被严格拒绝的测试；GREEN8项通过。未修改read_tsv的严格拒绝规则。

源码部分使用locked/offline/no-deps真实Cargo metadata，原verify_source_surface与verify_no_duplicate_trait_tree扫描721生产Rust文件通过。日志task6-final-source-deletion-2026-10-07.log；只覆盖源码，不作为API/rustdoc/runtime完整通过。

## 公共API主机修复

两个API入口原先硬编码nightly-2026-08-07-aarch64-apple-darwin，在当前Windows不可执行。实际自测RED9项/2失败（两个入口），改为同日期nightly-2026-08-07，由rustup选择host后GREEN9项通过。固定日期、all-features/-sss、API快照严格比较规则均保留。日志task6-public-api-host-red/green-2026-10-07.log。

这些自测验证调用参数与输出处理，不等于真实API生成通过。本机CLI/MSRV/nightly缺失已有只读证据，安装方案另见task6-final-gate-tool-installation-proposal-2026-10-07.md，尚未安装。

## C2 实际执行终态与输出恢复

session37151终态exit1：验证器无法确认task6_buffered_result_set_canonical_matrix的执行标记。1090冻结输入全部一致；无聚合测试计数可用，不能猜测已执行目标总数。完整日志SHA105c3c745453222ea2247002d7f1e52eb3b48c9adb8826f05cdc7f0d82f6b527，summary保留。

完整目标按原--nocapture/单线程独立复现exit0，7PASS0FAIL；真实stdout把TASK6_CASE_DIGEST插在“test function ...”与“ok”之间，旧严格函数解析因此拒绝。它不是产品断言RED。

新增验证器回归实际RED18项/1ERROR；保持严格成功函数与案例digest解析，只将运行选项改为--show-output后GREEN18项通过。同一实际完整目标--show-output再次exit0，7PASS0FAIL；按正式台账核对精确成功函数与32个参数化digest全部通过。两份实际目标日志和输出核对脚本保留，测试未删除、超时未扩大、规则未放宽。

完整C2已重新启动：LIVE session33663，日志task6-complete-stat-close-error-runtime2-2026-10-07.log；1090输入重新冻结，默认1.97.1/all-features/locked/offline/Cargo与测试线程各1。没有终态或整体通过声明，须继续同一句柄。

## C1 方法证据重新绑定准备

方法验证器自测18项通过。session18620完整--no-execute终态exit1：PhysicalConnection::execute仍指向补充default测试，但当前C2反向索引已有receiver-exact原测试来源。

session16536审计全部479行，仅1行需重新绑定至真实lease_complete_tests的task6_lease_delegates_complete_spi。候选JSON/TSV位于drafts/task6-method-rebinding-candidate-20261007.*；只改变候选behavior_evidence，其他BASE签名/目标/转换字段逐项保持。正式台账尚未修改、候选NOT_BOUND/NOT_EXECUTED，不计入验收。

后续须完整核对候选合同，并保留原补充default测试；当前C2运行期间不得修改Rust构建输入。没有把方法门禁失败豁免为通过。

历史全workspace3795/11、Admin49/11继续保留；没有当前全仓PASS或网络故障已解决声明。未stage/commit/push/install/publish或修改系统安全配置。

## 后续终态更正

上文session33663已终态exit1：offline缺失锁定arrow58.4.0；1090冻结输入一致，没有聚合测试计数。锁定依赖缓存fetch实际exit0后，完整C2在session13316/runtime3重新执行。上文单一C1候选在完整静态检查中仍FAIL，额外发现六项目标签名参数名记录漂移，新候选完整验证session97260运行中。后续完整证据见task6-method-evidence-preservation-2026-10-07.md；不能沿用此前LIVE或准备状态作为通过证明。
