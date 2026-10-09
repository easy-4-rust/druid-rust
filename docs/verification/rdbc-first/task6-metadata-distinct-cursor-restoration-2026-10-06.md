# Task6 独立游标错误与完整元数据恢复 — 2026-10-06

## 准确恢复范围

标准非SQLx缓冲适配器的value在未定位时恢复固定BASE的InvalidArgument(result set has no current row)。object/object_as入口保留原Other(result set cursor is not positioned on a row)，关闭和失效租约优先返回ConnectionDiscarded。定位后的Custom保留result_set_get_object_typed_custom。既有类型转换提取到唯一标准default_object_as自由函数；不复制分派、不增加第二驱动接口树。真实pooled旧租约案例仍验证实际创建语句exception_count3和相同底层连接/connection id19的新租约可用。

完整列元数据案例保留原24字段、10类类型各4项身份、三态代码；BASE body SHA为2e39d689dc3ebcfc298b88d3dcb0ee2382b9e7529c36f928bc6d33ef017005c9。仅恢复标准枚举五个查询。Jdbc保持原code，Named保留厂商名称；无法推导的类名和signed为Object/false，不覆盖真实descriptor的厂商名、类名及signed，也不改变ColumnDescriptor.new中性默认。

## 实际失败到通过证据

- task6-distinct-cursor-red.log：正式真实SQLite运行exit101，1PASS/2FAIL，错误类别不符。
- task6-distinct-cursor-green.log：exit0，3/3PASS，包含未定位、定位后Custom、关闭及失效租约。
- task6-complete-metadata-formal-red.log：正式Cargo编译exit101，17处E0599，缺失五个查询；属于编译缺失证据，不是运行时RED。
- task6-metadata-distinct-cursor-green.log：exit0，Druid lib94+FilterStat21+buffered matrix6+complete metadata2+typed object9=132PASS/0FAIL。
- 纯度初次拒绝新增matches宏缺少来源；准确改为std::matches!，未放宽验证器。
- task6-metadata-rdbc-related-green.log：exit0，四目标444PASS/0FAIL。
- task6-metadata-wrapper-related-green.log：exit0，3PASS/0FAIL。
- task6-metadata-final-binding-green.log：exit0，lib94+buffered matrix6+complete metadata2+typed object9=111PASS/0FAIL，包含完整错误计数/旧租约合同。

## 账本与门禁

只有完整元数据一行从参数化占位改为准确正式测试绑定：crates/druid/tests/task6_complete_metadata_contract_test.rs#task6_result_set_metadata_preserves_complete_rdbc_column_contract。只移除此案例的旧矩阵ID，反向调用索引和79行派生文档更新。固定BASE1283条前七字段全部核对未变。完整观察审计1236PASS/47FAIL；exact1099/parameterized184表示绑定方式，不代表全部行为合格。

完整C2 --no-execute仍exit1，下一项row_set_default_getters_cover_null_label_conversion_and_stream_contracts。该项目完整14列/成功失败stream三个helpers和最后共享转发身份不可删减。源码观察通过不能替代正式运行。

验证器自测15/15、标准纯度44、迁移权威79/Task7 pending25/Physical disposition73及原SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc、生成方法证据14文件均PASS。首次自测错误调用方式导致Python import失败，改为脚本实际入口后15/15通过；不是产品行为RED。

此前完整workspace85349终态exit101/242结果块/3712PASS/3FAIL/0ignored，两个Callable创建5vs6失败和一个value错误类别失败；此日志早于本修复，不是当前源码全仓通过证据。新的完整回归单独记录，不能推算其结果。

公开default_object_as与五个枚举查询的完整API/SemVer、默认1.97.1/MSRV1.95、all-features、Java差分、多数据库和最终审查门禁仍未关闭。Task6 FIX_REQUIRED、Task7八组件、Task8与P05–P07合计18个顶层任务保持开放。未创建分支、stage、commit、push、publish或安装工具。
