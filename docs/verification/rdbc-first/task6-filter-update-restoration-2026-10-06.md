# Task 6：ResultSet Filter 更新完整 BASE 合同恢复

## 原始要求与恢复范围

固定 BASE 的两个完整案例是 `result_set_scalar_update_filter_defaults_delegate_all_java_overloads` 和 `pooled_result_set_routes_lob_stream_updates_and_preserves_sqlite_capability_errors`。保留原 28 次标量下标/列名调用及 9 项 assert_eq，保留原 12 次 LOB 短路、资源余量、精确错误、真实 creating Statement 异常计数 1，以及 SQLite pass-through 的 12 次精确拒绝和计数 12。未降低原断言、BASE SHA 或 1283 分母。

原参数化分支用通用 Unsupported 检查代替成功委托和短路，因此即使矩阵通过也不能证明原合同。

## 实际失败与实现

- `task6-filter-update-capability-red.log`，同一 session36976 终态 exit101：26 PASS / 1 FAIL。完整标量案例通过；LOB 案例在真实 SQLite 第一个 pass-through 错误失败，实际 `result_set_update_value`，原期望 `result_set_update_blob_stream`。此前 12 次短路、调用日志、资源余量及 Filter 错误/Statement 计数 1 已执行通过；尚不能据此证明后续 12 次失败计数。
- ResultSetFilter 和 StandardResultSetFilterChain 添加 canonical 下标/原列名更新 around hook，默认递归委托同一标准 ResultSet。池化入口保留外层关闭/租约门禁、列治理、策略分类和 ResultSet 错误统计，并向实际创建语句记一次异常。
- canonical ResultSet 默认根据 BlobStream/ClobReader/NClobReader 描述符保留六个原始下标/列名错误名称。列名默认 LOB 拒绝不查列，不读取或消费资源；其他更新仍保留原默认路径。长度变体共享原操作名称，未增加第二套驱动端口。

## 当前验证边界

修改的两个 RDBC 文件格式检查和 git diff --check 通过。新的同范围运行正在 session56316 执行，日志 `task6-filter-update-lob-default-green.log`；未经句柄终态和完整断言通过不得标记 GREEN 或绑定为完成。

后续只读核查补充：使用现有验证器从固定 BASE 重新推导全部案例，再检查两个新完整目标，session81719 终态 exit0 / SELECTED_BASE_SOURCE_FLOOR=2/2。LOB 原 `assert:1;assert_eq:8` 与新目标相同，标量原 `assert_eq:9` 与新目标相同，调用数量门禁也通过。日志 `task6-filter-update-selected-floor.log`。这不是运行证明或完整 C2 通过，不修改账本处置或分母。新标准纯度检查 session95041 终态 exit0 / STANDARD_FILES=44 / PURITY_PHASE=final；差异检查 session2446 终态 exit0。运行验证仍保留同一 session56316，rustc 已推进到下一编译进程，未重启。

新完整案例目前尚未修改迁移账本绑定；旧矩阵中的两个占位分支及其通用拒绝预期须在完整运行通过后移除/校准。外部 Filter 扩展性、多个 Filter 的顺序和短路、关闭租约及错误分类回归仍待检查。

上一完整 workspace 3704 PASS / 2 FAIL 属于此次 Filter 改动前，两个 Callable 原始计数 5 vs 6 仍开放。Task6 FIX_REQUIRED、完整 C1/C2、元数据身份、Task7 八组件、Task8 和 P05–P07 共 18 个顶层任务不变。

## 完整原案例通过及准确绑定

同一 session56316 终态 exit0，`task6-filter-update-lob-default-green.log`：27 PASS / 0 FAIL / 0 ignored。两个新完整原始案例实际通过；包含 28 次标量原值委托、12 次 LOB 短路、调用者资源余量、精确 Filter DriverError、真实 Statement.exception_count=1，以及 12 次 SQLite 精确拒绝、真实 exception_count=12、游标未关闭。这一终态替代上文 LIVE56316，未证明其余 Filter 原案例或整体 Task6 完成。

仅将这两个 BASE 行改为 MIGRATED_EXACT，指向 diagnostic_tests 的完整 lib 测试；验证器只加入这两个具体名称白名单，不扩大到整个测试模块。移除旧矩阵两个占位 case ID 和对应 grouped arm 名称，其他待恢复案例不冒充完成。账本现在 1092 exact / 191 parameterized，共 1283，原 BASE 元信息不变；反向索引和派生文档已同步，格式与差异检查通过。

迁移权威表 79 行、Task7 pending25、Physical disposition73及固定 SHA 检查终态 exit0。首次以 unittest 模块方式执行自测由于脚本导入路径错误而失败，未修改测试或验证器规避；已改用脚本自身入口，session27486 待终态。完整 C2 session76053 和相关 Druid lib/FilterStat/Metadata/Buffered 回归 session18927 正在运行，未经终态不得声称通过。无 Rust 源码编辑与运行并发。

后续终态：session27486 exit0，验证器自测15/15；session18927 exit0，6结果块/134 PASS/0 FAIL/0 ignored，包括 lib87、FilterStat21、metadata3、buffered6、FilterCoverage16、FilterMatrix1。lib87 包含此前的27个诊断测试，不重复计算为161。session76053 exit1，完整 C2 仍首败 `physical_result_set_metadata_preserves_getter_error_and_wrapper_identity` 唯一明确分支。这两项完成绑定不关闭完整 C2。

完整 workspace locked/offline/serial/no-fail-fast 回归已启动 session41767，日志 `task6-filter-update-workspace.log`；未经终态不以旧3704/2或新134/0推断全量结果。保持进程级F盘构建目录，不删除缓存或更改Cargo配置。

Task6 Filter workspace终态核查 2026-10-06：会话41767已不在工具注册表；cargo51544进程不存在，日志包含完整Doc-tests及最终2 targets failed，确认终态失败，不重启原命令。242结果块/3705 PASS/3 FAIL/0 ignored，含2doctests。两项Callable仍prepared5vs6；新增strong_getter_default_contract_test的缓冲LOB列名拒绝失败（sql_exception=None），不能沿用旧3704/2结论。源码证明新只读默认列名提前拒绝绕过BufferedRowSet原scalar_update的0A000；恢复该驱动的列名委托，保留原测试。Callable创建/拒绝/关闭源码审计详见task6-callable-admission-source-audit-2026-10-06.md，不修改计数预期或失败计数规则。完整18任务/Task6 FIX_REQUIRED保留。
