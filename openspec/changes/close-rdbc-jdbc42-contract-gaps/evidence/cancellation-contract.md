# Statement 独立取消入口：任务 4.1

新增 `StatementCancellation` 标准异步接口和 `Statement::cancellation_control`。调用方可在执行前取得 `Arc` 取消句柄；执行 Future 独占借用非 `Sync` Statement 时，另一任务通过句柄发起取消。旧 `Statement::cancel(&mut self)` 保留兼容；默认新接口明确返回 SQLState `0A000`。

旧入口的借用限制由 `Statement::cancellation_control` 的 compile-fail doctest 记录；示例在 Future 持有 `&mut Statement` 时再次调用 `cancel(&mut self)`，必须编译失败。新合同覆盖一项实现了 `Cell` 字段（因此非 `Sync`）的外部集成测试，执行挂起后从另一个任务调用取消句柄并验证 `HY008` 结果；另有最小 Statement 实现验证默认 `0A000`。

验证：

- `cargo test -p druid-rdbc --test statement_cancellation_control_contract_test --locked --offline`：2 passed。
- `cargo test -p druid-rdbc --doc --locked --offline`：2 passed，其中 1 项是预期编译失败的旧借用示例。
- 本任务仅关闭公共控制面合同；owner/租约/执行代次、不确定状态屏障、SQLx 真实执行取消仍由 4.2–4.4 实现，不能据此宣称取消功能已完成。
