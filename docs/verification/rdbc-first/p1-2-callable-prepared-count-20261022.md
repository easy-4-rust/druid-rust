# P1-2 Callable prepared 计数 5 vs 6（2026-10-22）

- Java 基线：Druid 1.2.28 @ `fa8dc9912637a2f729eef9f55356621fec18d40e`
- Rust 基线：druid-rust @ `ebb03aff018db9a9660a1dded7e16254196c155f`
- 关联：`status.md` 的 Task6 Callable `prepared 5 vs 6`；`D03` 已批准差异

## 1. Java 原语义

| 计数 | Java 位置 | 语义 |
|---|---|---|
| `preparedStatementCount` | `DruidAbstractDataSource#incrementPreparedStatementCount`（554） | `DruidPooledConnection` 在 `conn.prepareStatement/prepareCall` 成功返回、`initStatement` 之前递增；失败不回滚 |
| `closedPreparedStatementCount` | `DruidAbstractDataSource`（546） | 由 `DruidPooledConnection`（197）在 statement 关闭时递增 |
| `cachedPreparedStatementDeleteCount` | `DruidAbstractDataSource`（514） | 缓存删除时递增 |

## 2. 核对结果

- 当前 Rust 实现的 6 条计数路径与冻结对照一致：**prepared = 6、closed = 6、delete = 6**。
- 第 6 次来自“驱动返回非 Callable 句柄”的拒绝路径：holder 先创建并计数，校验失败后立即关闭，因此 closed/delete 同步到 6。
- 第 7 次 `prepare_call("unsupported")` 在驱动层失败，不产生 holder，计数保持不变。
- 结论：**生产代码无需修改**；本轮补齐的是“逐次归属断言”，避免该计数只被大用例末尾的汇总断言覆盖。

## 3. 新增断言

文件：`crates/druid/tests/task6_callable_canonical_matrix_test.rs`（`task6_callable_errors_invalidate_cache_and_non_callable_handles_are_rejected`）。

在 5 个成功 prepare/close 步骤后分别断言 `prepared/closed/delete = 1..5`；在非 Callable 句柄拒绝后断言 `6/6/6`；在 unsupported 失败后仍为 `6/6/6`。

## 4. 验证

```bash
cargo test -p druid --test task6_callable_canonical_matrix_test
rustfmt --edition 2021 --check crates/druid/tests/task6_callable_canonical_matrix_test.rs
```

结果：10 passed / 0 failed；rustfmt exit 0。

## 5. 未闭合

- 拒绝路径的 `closedPreparedStatementCount` 归属只有 Rust 回归断言，尚未与 Java 同路径做逐值差分；
- 该用例只覆盖计数，不覆盖 OUT 参数顺序、命名参数与 ResultSet 身份（D03 其余项）。
