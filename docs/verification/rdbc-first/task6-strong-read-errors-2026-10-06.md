# Task 6：强类型读取错误与创建语句异常统计（2026-10-06）

## 范围与原始合同

完整目标仍为 P04 Tasks 6–8 与 P05–P07，18 个顶层任务未闭环。本轮前置架构核查取得新的 C2、纯度、权威表与强类型运行失败证据，下一步据此恢复原始错误路径。

固定 BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e 的 strong_typed_getter_conversion_errors_follow_statement_check_exception_path 要求：单列 event_date=not-a-date，next 成功；date_by_label 保留含 cannot be converted to Date 的 DriverError；blob(1) 保留 UnsupportedOperation(result_set_blob)；真实创建 Statement 的 exception_count 为 2。四项原断言完整保留。

## RED 与实现

- task6-strong-read-error-red.log：运行时原案例 0 PASS / 1 FAIL，Blob 返回错误家族不符合 BASE。
- task6-resource-read-default-red.log：补充八类资源默认请求运行时 0 PASS / 1 FAIL，Ref 请求把普通字符串返回为成功资源读取。
- task6-strong-read-count-red.log：同一 session68447 终态 exit101 / 0 PASS / 1 FAIL；资源家族修复后原计数实际 0，原期望 2。

canonical ResultSet::read_object 的默认资源分支校验 Ref/Blob/Clob/Array/URL/RowId/NClob/SQLXML 家族，保持准确不支持操作名称；返回原资源或 SQL NULL，不克隆或消费资源，保留前置驱动错误。DruidPooledResultSet 两个 read_object 入口保留 Filter、原列名、既有列治理和 access_core 策略分类，再向实际创建语句记录一次读取错误。关闭/租约失效检查仍在外层，不执行额外 driver 读取，不重复分类。

本轮修正了新增资源 helper 与日期转换函数的中文 doc 注释归属。未新增标准接口或第二套驱动端口。

## 新鲜验证证据

- task6-strong-read-green.log，session76784 终态 exit0：Druid lib85 + 类型默认合同9 =94 PASS /0 FAIL。
- task6-strong-read-rdbc-green.log，session98840 终态 exit0：16 结果块 /471 PASS /0 FAIL，含1doctest；生成合同测试不能替代 C1 全生产行为证明。
- task6-strong-read-regression.log，session64357 终态 exit0：FilterStat21、metadata3、buffered6、FilterCoverage16、FilterMatrix1，共47 PASS /0 FAIL。本轮 Druid 合计141 PASS。
- 选定原始案例的 verify_observation_floor 实际通过；只增加这一项准确 private lib 测试白名单，原 BASE 身份、SHA、关键断言/调用和1283分母不变。旧矩阵占位 case ID 已移除，反向方法索引与派生迁移文档同步。
- C2 --no-execute，session51315 终态 exit1：仍缺 physical_result_set_metadata_preserves_getter_error_and_wrapper_identity 的唯一明确分支。完整 C2 仍 FAIL。
- 账本1090 exact /193 parameterized；最低缺明确分支54：buffered16、lease2、recycle12、ResultSetFilter24。数量不是行为完成率。
- 验证器自测15/15、final purity44、authority79/Task7pending25/Physical disposition73及固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc通过。修改的6个Rust文件格式检查及 git diff --check通过。

## 全量回归与剩余要求

本轮前的 object-label workspace 回归已终态 exit101 /242结果块 /3702 PASS /2 FAIL，仅 Callable 原prepared5vs6失败；早于本轮源码。

本轮 task6-strong-read-workspace.log 终态 exit101，在编译 druid-rdbc archive 时因 E盘空间不足 os error112 中止，尚无测试执行结果。只读检查当时 E盘剩61837312字节。不能把环境编译失败冒充新的行为回归结果。

已使用进程级 CARGO_TARGET_DIR=F:/codex_build_cache/druid_rust/task6_strong_read_20261006 启动完整 locked/offline/serial/no-fail-fast workspace 回归，session42807，日志 task6-strong-read-workspace-f-drive.log。未删除、移动原target或修改Cargo配置。该独立目录需要重新编译，未经同一工具句柄确认终态不得声称全量通过；等待期间不修改Rust。

Task6仍FIX_REQUIRED。元数据底层identity、对象Filter/资源统计、Callable原计数、C1/C2、公共API/compiler/SemVer/allfeatures/MSRV及最终审查继续开放。Task7八组件、Task8和P05–P07保留。无分支、提交、推送、发布、安装、目标完成或阻塞声明。

完整回归终态补充：同一F盘session42807已由工具句柄确认exit101，242结果块/3704PASS/2FAIL/0ignored含2doctests。仅原Callable两案例prepared5vs6失败。替代本报告LIVE记录，该终态早于随后新增的Filter测试源码；不证明Task6或完整目标完成。
