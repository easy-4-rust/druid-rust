# Task 6 标准语句身份与 Java 缓存统计迁移

规格：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md` Task 6。
BASE：`5afe860:crates/druid-wrapper/tests/prepared_statement_semantics_test.rs`。

## 本轮交付

恢复 `prepared_statement_reuses_physical_handle_and_updates_java_stats` 完整显式 canonical arm，并增加独立测试 `prepared_reuse_preserves_standard_identity_and_java_cache_statistics`。没有生产代码改动。

验证标准 Wrapper/Statement/Prepared 视图、未知目标拒绝、空目标参数错误、数据对象地址一致、真实逻辑句柄关闭后的标准叶子身份、查询读取、缓存后无底层关闭、第二句柄不重新 prepare、更新执行及最终池关闭恰好一次。

完整保留 BASE 五个 Java 统计量：prepared_statement_count=1、cached_count=1、hit_count=1、miss_count=1、access_count=2。额外以驱动 `connection.prepare:select 1` 事件数量=1 验证实际复用。

Ruling: 根据已批准的 single-standard 合同，旧 Physical/private SqlText unwrap 映射为标准 Prepared 叶子与同一对象数据地址；不重新暴露被删除的内部类型。null 目标按已有规范返回 InvalidArgument，不恢复旧 false/None。缓存 holder 私有 in_use/pooling 状态通过逻辑关闭状态、缓存计数及底层关闭/prepare 事件共同验证，不引入第二套公共接口。

## 验证记录

- 首次运行 9 PASS/2 FAIL：`std::ptr::eq` 比较了 trait object 数据与 vtable。标准转换路径可产生不同 vtable，这不是对象身份变化或产品 RED。改为 `std::ptr::addr_eq`，仅比较原对象数据地址。
- 随后验证编译失败：另一个确认存活的 cargo clean 进程（PID 48452，目标为本仓库 target）清理了 deps。没有终止其他任务；确认该进程结束后重新运行。失败日志 `task6-reuse-statistics-runtime-final.log` 保留为环境失败，不计为产品 RED。
- 最终重建运行 exit 0，11/11 PASS，日志 `task6-reuse-statistics-rebuilt.log`。本轮不宣称生产 RED→GREEN：现有生产代码已满足新增完整用例。
- C2 no-execute exit 1，当前下一缺失显式 arm：`prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite`，日志 `task6-reuse-statistics-c2.log`。没有降低断言门槛或重写迁移分母。
- purity final exit 0，standard files 43；authority exit 0，79 行、Task 7 pending 25、SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`。目标文件 rustfmt exit 0。
- 上一轮完整 workspace 3634 PASS/0 FAIL 的生产源码本轮未变；该结果不作为新增测试源的最新完整回归。本轮最新证据是目标矩阵 11/11。

## 后续与未完成

下一完整 BASE 用例包含真实 SQLite 多种资源绑定、短流/负长度错误、setter 返回前资源消费时机、query/generic/batch 执行与 Filter 富描述符快照，不能缩成只验证最终 SQL 值。

Task 6 仍 FIX_REQUIRED，C1/C2/API/compiler/SemVer/features/MSRV/full review 未关闭；完整 Task 7–8/P05–P07 目标保留。未 stage/commit/push/publish/install。
