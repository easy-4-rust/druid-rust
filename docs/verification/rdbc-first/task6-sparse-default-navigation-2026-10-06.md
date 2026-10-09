# Task6 稀疏默认导航合同恢复（2026-10-06）

固定 BASE5afe8609b177544ff7d18461aa84f69efe8ee90e，原案例 sparse_result_set_navigation_defaults_report_exact_capabilities，body SHA3537dd4e1b76ba4019ef85bf6e78b48afd8e52ab48b1aab983d07ef6f376eb4f。

原 SparsePhysicalResultSet 仅提供 close/is_closed，原十个拒绝来自接口默认。迁移支持对象 SparseDefaultResultSet 只实现标准必需 next 和原关闭合同，不覆盖 navigate 或其他可选能力。因此测试证明真实默认方法，不以测试 fixture 自行拒绝伪造迁移。完整 next/previous/first/last/before_first/after_last/absolute(-2)/relative(3)/row/value(1) 均保留。

实际运行 RED0PASS1FAIL：previous 返回通用 SQL 导航错误，未保留原 UnsupportedOperation(result_set_previous)。随后 canonical navigate 按七种非Next variant 返回精确 UnsupportedOperation，Next 保持委托 mandatory next。具体驱动覆盖方法仍可报告自己的 SQL0A000。正式测试 GREEN1PASS0FAIL，源观察原10调用全部通过。新要求记录在既有单标准规格，不创建第二套规格或驱动端口。

日志：task6-sparse-default-navigation-red.log、task6-sparse-default-navigation-green.log。

扩大回归发现旧 boolean 补充测试关闭后预期 ConnectionDiscarded，与 BASE boolean -> value -> ensure_open 的精确 Other(result set is closed) 冲突。只修正该期望，保留全部18转换及真实游标/NULL/错误观察；失败记录保留在 task6-sparse-default-navigation-related.log，不改生产错误以迎合旧补充断言。

固定1283账本仅更改一个完整案例绑定和相应参数化case列表；1102exact/181parameterized。完整源码审计1239PASS44FAIL，全部前七基线字段不变。C2 --no-execute exit1 下一 sparse_result_set_property_defaults_report_exact_capabilities。

相关回归 task6-sparse-default-navigation-final-related.log：RDBC完整包已终态exit0/16结果块/471PASS0FAIL，含1doctest。同一会话58225最终exit0：Druid八目标142PASS0FAIL，Wrapper两目标5PASS0FAIL；相关总计618PASS0FAIL，26个结果块。完整workspace新鲜验收仍待执行，不以本相关范围代替。

自测15/标准纯度48/权威79行及25待处理/Physical映射73/生成14/定向格式/diff检查通过。公共行为与API/默认工具链/MSRV/features及整体审查门禁仍待完成；上一完整workspace3716PASS2FAIL属于更早源码。Task6仍FIX_REQUIRED，完整18顶层任务与Task7八义务保持开放。
