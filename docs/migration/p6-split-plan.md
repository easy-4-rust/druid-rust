# P6 结构债拆分计划（28 个超长文件）

> 生成来源：`docs/migration/inventory/redlines.tsv`（2026-10-22，Rust 基线 `ebb03af`）。
> 目标：把 authored `.rs` 压到 ≤800 行；501–800 行逐条记录 cohesion review；一文件一 Java 对象。

## 1. 约束（先于拆分）

1. Rust 一个类型对同一 trait **只能有一个 impl 块**。因此“把 `impl Connection for DruidPooledConnection` 拆成两个文件”不可行；只能把**辅助函数/固有方法**移出，或申请结构性例外。
2. `lib.rs`/`mod.rs` 只做声明与重导出；子模块必须承载真实逻辑，不得用 `include!` 规避行数。
3. 每批拆分必须：`cargo check`、目标测试、`cargo test -p <crate> --tests`、`rustfmt --check` 全绿后再进入下一批。
4. 拆分提交必须单独成 commit，禁止与语义修复混提。

## 2. 批次与目标模块

### B1 测试代码（最低风险，先做）

| 文件 | 行数 | 目标 |
|---|---:|---|
| `crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs` | 10,000 | 按行为族拆到 `diagnostic_tests/{metadata,scalar,update,lob,cursor,filter}.rs`，主文件只保留 `mod` 与共享夹具 |
| `.../diagnostic_tests/filter_defaults_chain_harness.rs` | 808 | 拆为 harness + 断言辅助两个文件 |

验收：`cargo test -p druid --lib` 中该模块用例数与拆分前一致（逐用例 ID 不变）。

### B2 wrapper 适配器（高内聚，按资源族拆）

| 文件 | 行数 | 目标 |
|---|---:|---|
| `druid-wrapper/src/sqlx/sqlx_connection_adapter.rs` | 1,975 | `{connection,metadata,transaction,lob}.rs` |
| `druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs` | 1,335 | `{connection,metadata,lob,result}.rs` |
| `druid-wrapper/src/driver_admin/driver_installer.rs` | 1,099 | `{install,verify,manifest}.rs` |
| `druid-wrapper/src/sqlx/sqlx_prepared_statement.rs` | 1,058 | `{bind,execute,metadata}.rs` |
| `druid-wrapper/src/sqlx/sqlx_connection_actor.rs` | 992 | `{command,lifecycle}.rs` |
| `druid-wrapper/src/toasty/toasty_connection_adapter.rs` | 991 | `{connection,statement,result}.rs` |

验收：SQLite/Toasty/SQLx 真实适配器测试不变。

### B3 核心池（先移固有方法，再评估 trait impl 例外）

| 文件 | 行数 | 目标 |
|---|---:|---|
| `druid/src/pool/pool_inner.rs` | 1,796 | 拆 `{borrow,return,create,shrink,stats}.rs`（同一 `impl PoolInner` 可分文件） |
| `druid/src/pool/druid_pool.rs` | 1,777 | 拆 `{acquire,lifecycle,stats}.rs` |
| `druid/src/pool/config.rs` | 1,208 | `{pool_config,builder,properties}.rs` |
| `druid/src/core/druid_pooled_connection.rs` | 2,253 | 固有方法可拆；**trait impl 约 930 行不可再分**，需结构性例外或 helper 下沉 |
| `druid/src/core/druid_pooled_result_set.rs` | 1,690 | 按 getter 族拆；trait impl 同样受单一 impl 约束 |
| `druid/src/core/druid_pooled_statement.rs` | 1,353 | `{execute,property,batch}.rs` |
| `druid/src/core/druid_pooled_callable_statement.rs` | 1,378 | `{out_params,execute,metadata}.rs` |
| `druid/src/core/druid_pooled_prepared_statement.rs` | 1,208 | `{bind,execute,cache}.rs` |
| `druid/src/core/filter_chain_impl.rs` | 1,333 | `{connection,statement,result_set,lob}.rs`（链结构体天然分组） |
| `druid/src/core/standard_resource_registry.rs` | 1,308 | `{registry,coordinator,state}.rs` |
| `druid/src/core/xa.rs` | 1,013 | `{state,xid,recovery}.rs` |
| `druid/src/core/log_filter.rs` | 966 | `{config,events}.rs` |
| `druid/src/sql/wall.rs` | 983 | `{rules_dml,rules_ddl,tenant}.rs` |
| `druid/src/sql/lexer.rs` | 1,072 | `{scan,keywords,features}.rs` |

### B4 rdbc / metrics / admin

| 文件 | 行数 | 目标 |
|---|---:|---|
| `druid-rdbc/src/result_set/result_set_ext.rs` | 1,216 | 按 getter 类型族拆（trait 默认方法可保留单一 impl） |
| `druid-rdbc/src/extensions/row_set.rs` | 1,128 | `{cursor,row_store,metadata}.rs` |
| `druid-rdbc/src/extensions/shared_row_set.rs` | 1,060 | `{borrow,state,metadata}.rs` |
| `druid-metrics/src/push_worker.rs` | 1,424 | `{ack_window,resend,metrics}.rs` |
| `druid-metrics/src/grpc_client.rs` | 963 | `{connect,frames}.rs` |
| `druid-admin/src/servlet/monitor_view_servlet.rs` | 1,420 | `{pages,json_api,auth}.rs` |

## 3. 执行顺序与门禁

1. **B1**（只动 `#[cfg(test)]`）→ 验证用例 ID 与数量不变。
2. **B2**（wrapper）→ 真实 SQLite/SQLx/Toasty 测试。
3. **B3**（pool）→ 先 `pool_inner`/`druid_pool`（固有方法），再评估 connection/statement 的 trait impl 例外申请。
4. **B4**（rdbc/metrics/admin）→ 合同测试 + admin HTTP 测试。
5. 每批完成后重跑 `scripts/migration_inventory.py` 与 `render_migration_docs.py`，确认 `FILE_OVER_800` 计数下降。

## 4. 已知阻塞

- **trait impl 原子性**：`DruidPooledConnection`、`DruidPooledResultSet`、各 adapter 的 trait impl 单块超过 800 行，无法在不改变类型/合同的前提下拆成多个 impl 文件。处理方式二选一：
  1. 把方法体委托给 helper（helper 放子模块），使 impl 块变薄；
  2. 提交“结构性例外”：记录对象、行数、不可拆原因、审批人与复审日期。
- **例外不得静默**：在例外被批准前，对象表继续把该文件计为 `FILE_OVER_800`，路线图 P6 不得标记完成。

## 5. 执行进度

- 2026-10-22（首轮尝试）：对 `druid_pooled_connection.rs` 做“固有方法 + trait impl”拆分，验证了 **trait impl 单块约 930 行**这一硬约束；为避免把 1 个超长文件变成 2 个超长文件，已**回滚**该拆分。
- 2026-10-22（B1 第一批，已提交）：
  - `diagnostic_tests/filter_defaults_chain_harness.rs` 808 → 385 + 433（按读取值族拆出 `decorated_reads.rs`）；
  - `core/log_filter.rs` 966 → 578 + 401（拆出 `options.rs` 承载 Java 同名 getter/setter）。
  - 超 800 行文件：**28 → 26**；`cargo test -p druid --lib` 161 passed。
- 2026-10-22（B1 第二批，已提交）：
  - `diagnostic_tests.rs` 9,851 → 父模块 121 行 + `harness.rs` 159 行 + **17 个行为族模块**（每个 ≤800 行，最大 797）；
  - 81 个测试逐用例保留（`cargo test -p druid --lib diagnostic` = 81 passed），共享 SQLite 夹具集中到 `harness.rs`；
  - 超 800 行文件：**26 → 25**。
- 2026-10-22（B2 wrapper 批次，已提交）：
  - `sqlx_connection_adapter.rs` 1,949 → 475 + 764 + 747（`adapter_core` / `adapter_queries`）；
  - `sqlx_prepared_statement.rs` 1,041 → 554 + 307 + 210（`statement` / `prepared_statement`）；
  - `sqlx_connection_actor.rs` 952 → 781 + 60 + 149（`sqlite_cleanup_guard` / 测试模块）；
  - `toasty_connection_adapter.rs` 975 → 640 + 358（`standard_connection`）；
  - `driver_installer.rs` 1,099 → 40 + 505 + 565（安装 / 校验与清单）。
  - 跨模块可见性统一为 `pub(crate)`；`#[async_trait]` 随 impl 一起迁移。
  - `cargo test -p druid-wrapper --all-features --lib` 69 passed；`clippy -D warnings` exit 0；
  - 超 800 行文件：**25 → 20**。
- 未完成：`jdbc_agent_connection.rs`（1,329）首次拆分因属性/可见性边界失败，已回滚，留待下一批用“support/impl 分文件”方案重做。
- 2026-10-22（B3 + B4，已提交）：
  - `pool_inner.rs` 1,796 → 292 + 581/426/500（state / create / recycle）；
  - `druid_pool.rs` 1,777 → 363 + 247/417/753（construct / acquire / state）；
  - `config.rs` 1,208 → 321 + 438/450（builder / builder_advanced）；
  - `monitor_view_servlet.rs` 1,400 → 536 + 434/451；
  - `push_worker.rs` 1,424 → 460 + 469/496；
  - `shared_row_set.rs` 1,060 → 413 + 649（trait impl 分离）。
  - `clippy -D warnings`（三 crate）exit 0；lib tests：rdbc 5、metrics 6、admin 6 全通过；
  - 超 800 行文件：**20 → 14**。
- 2026-10-22（B5：sql/core 中等文件，已提交）：
  - `xa.rs` 1,013 → 764 + 两个状态机子模块；
  - `lexer.rs` 1,072 → 170 + 两个扫描子模块；
  - `standard_resource_registry.rs` 1,308 → 505 + 两个注册表子模块；
  - `grpc_client.rs` 963 → 611 + `transport_helpers`（354）；
  - `wall.rs` 983 → 734 + `rules`（251）。
  - `jdbc_agent_connection.rs` 第二次拆分仍失败：批量提升可见性时命中了 trait 方法签名（`E0449`），已再次回滚；下次必须只对 `impl` 块内方法做提升，并显式重导出类型。
  - 超 800 行文件：**14 → 9**。
- 结构性例外候选：`rdbc/src/extensions/row_set.rs`（`trait RowSet` 单定义，1,128 行）与 `result_set_ext.rs`（`trait ResultSetExt` 单定义，1,216 行）无法在不改变公共 trait 形态的前提下拆文件，需按计划申请例外或改 supertrait 设计。
- P6 状态：**IN_PROGRESS**；`crates/druid-test` 整仓验收包在 B1–B3 完成后启动。
