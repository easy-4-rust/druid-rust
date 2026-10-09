# Task 6 跨租约缓存归还与 setter 描述符验证

对应规格：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`。
对应计划：`docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md` Task 6。

## 变更与判定

完整恢复 BASE `5afe860` 的 `prepared_statement_cannot_cross_connection_lease_boundary` 显式 canonical arm，并添加独立运行测试。种子语句缓存命中后，第一连接归还；旧逻辑句柄拒绝执行，第二租约安全复用缓存条目。底层 prepare 事件只有一次、旧句柄 Drop 不删除缓存、池关闭最终只关闭底层语句一次。

连接归还的 registry 原先直接调用底层 Statement view close，绕过共享 Prepared owner 的属性恢复与缓存归还。`DruidPooledStatementInner::close_resource` 现在先取得共享逻辑 owner 并通过同一标准 Statement close 策略关闭；没有存活 owner 时仍使用原始资源关闭。同步锁在 await 前释放，使用现有弱 owner，不引入强引用环。

Ruling: 标准拥有型 PreparedStatement 不再接收另一个连接作为执行参数；历史跨租约禁用通过旧拥有型句柄在第二租约期间执行失败验证，同时完整保留物理缓存复用要求。没有新增第二套连接/语句接口。

## 实际验证

- 初次运行 8 PASS/2 FAIL：补充的查询观察误用了普通 Statement fixture 的 11；Prepared fixture 源码明确返回 22。该失败是测试预期错误，不是产品 RED。
- 修正 fixture 预期后 8 PASS/2 FAIL：底层 prepare 次数实际 2、要求 1，是真实缓存归还行为 RED。日志 `task6-lease-boundary-runtime-final.log`。
- 修改 registry 关闭委托后目标测试 10/10 PASS、exit 0，日志 `task6-lease-boundary-green.log`。
- setter 全部 48 描述符保留及资源不提前读取也在本次目标矩阵执行。该测试通过执行前 Filter 快照验证绑定阶段，不声明所有富参数已完成底层驱动执行一致性。
- 新鲜 C2 `--no-execute` exit 1，推进到下一缺失显式 arm：`prepared_statement_reuses_physical_handle_and_updates_java_stats`。不是 C2 全部完成。
- purity final：43 文件通过；authority：79 行、Task 7 pending 25、SHA `16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc`。rustfmt 与 git diff --check exit 0。
- 完整 workspace 默认 locked/offline serial --no-fail-fast 回归终态：session 84985，exit 0，235 个结果块，3634 PASS / 0 FAIL / 0 ignored，包含 2 doctests。日志 task6-lease-boundary-workspace.log。

## 未完成范围

Task 6 保持 FIX_REQUIRED。完整 C1/C2、API/compiler/SemVer/all-features/MSRV 与最终审查仍未闭合。Task 7–8、P05–P07 的完整目标不变。未创建分支、worktree，未 stage/commit/push/publish/install。
