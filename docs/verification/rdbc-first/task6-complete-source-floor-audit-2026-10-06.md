# Task 6：全部原始案例的断言/调用数量核查（2026-10-06）

本轮前置为 verified wait：完整workspace同一session42807仍在F盘编译，工具句柄连续确认LIVE，实际rustc父进程为cargo67372。等待时只读取代码与执行现有验证器函数，没有修改Rust或重启编译。

## 范围与结果

对固定BASE全部1283函数逐项调用现有 verify_observation_floor / parameterized_case_body / verify_parameterized_observation_floor，而非在第一项缺口停止。未修改验证器或降低判定要求。

- 1090 MIGRATED_EXACT 项通过断言/调用数量检查。
- 193 MERGED_PARAMETERIZED 项中137通过数量检查，56失败。
- 56失败由54缺少唯一明确分支与2已有分支却削弱原始断言组成。
- 分布：Buffered16，Lease2，Recycle12，ResultSetFilter26。

这是绑定与原观察数量检查，不是全部C2、语义等价、生产行为或运行证明。54的原“最低缺明确分支”数量保持正确，但不能作为所有静态验收缺口总量。

## 两项新增明确缺口

1. pooled_result_set_routes_lob_stream_updates_and_preserves_sqlite_capability_errors：原 assert_eq 数量8，现分支2。当前 grouped arm 用通用Unsupported检查替代原真实ToastySQLite+自定义LOB短路Filter。原要求12个Blob stream/Clob Reader/NClob Reader下标/标签和长度重载，保留Some/None、负长度和i64::MAX；调用日志精确，Filter分别消费首字节/首字符，调用者随后读取资源余量；Filter错误保留准确DriverError并记入实际Statement一次；pass-through保留真实SQLite不支持操作名与未消费资源。

2. result_set_scalar_update_filter_defaults_delegate_all_java_overloads：原 assert_eq 数量9，现分支2。当前 grouped arm 用通用Unsupported检查替代原PassThrough Filter链的成功委托。原要求14个setter×下标/标签=28次末端调用，准确保留NULL、bool正反值、整数正负值、浮点、Decimal scale、String/Bytes/日期时间值与nullable对象，核对原日志的全部指定观察。

不能仅增加空assert_eq满足数量门禁；必须恢复完整原数据、返回值、资源消费、Filter顺序、错误和creatingStatement统计。

## 当前生产路径证据

CodeGraph查询并读取当前源：DruidPooledResultSet::update_value/update_value_by_label已保留驱动下标/原标签委托和列治理，但直接进入raw update，没有ResultSet Filter更新链。ResultSetFilter与StandardResultSetFilterChain没有对应标准update hooks。现有无Filter的全部getter/update身份测试不能证明这两项Filter合同已迁移。

update路径的 access_core 完成策略分类，随后 record_result_set_error 记录统计；后者并非再次fatal分类，不能凭方法名错误删除。恢复Filter时仍需保留分类与统计的各自责任，并证明创建语句异常计数一次。

下一实现需在唯一标准树内加入保留ResultSetUpdate完整描述符的Filter更新路径，区分下标/原列名，支持真实around短路与默认terminal委托，不恢复Physical或第二套驱动端口。先恢复两项完整BASE测试，观察真实RED，再实施并跑原始/相关回归。

完整C2仍首败metadata wrapper identity；此两项为后续隐藏失败，未替代、关闭或缩小metadata原案例。原BASE SHA、1283分母、账本处置及18顶层目标均保持。本报告本身不增加完成项。

明细：.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-source-floor-audit-2026-10-06.json。
