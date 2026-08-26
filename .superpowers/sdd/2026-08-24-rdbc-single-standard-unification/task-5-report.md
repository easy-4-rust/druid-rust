# P04 Task 5 报告：Druid holder 与 pooled proxies 统一使用 standard

## 1. 范围与基线

- BASE / 开始 HEAD：`d1ccc8e3005a3a71638766ee701af99594fc43ed`。
- 分支：用户授权的当前 `main`；没有创建 worktree、切换分支、reset、amend 或 push。
- 项目类型：Brownfield Rust workspace。
- 规格事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md` / ADR-RDBC-006。
- 执行 brief：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-5-brief.md`。
- 执行方法：延续现有 Superpowers SDD，严格 TDD；没有重新执行 Task 1–4 实现。
- Toolchain / MSRV / edition：Rust `1.97.1` / `1.95` / 2021。
- CodeGraph：仓库没有 `.codegraph/`，因此使用源文件、动态 verifier 和编译器测试证据。
- 开始状态：Task 4 `TASK4_COMPLETE_REVIEW_CLEAR`；Task 3 purity `103/103 + live29`。

本次没有修改 Cargo topology、依赖或 lockfile，也没有删除 Task 6 才负责删除的
`Physical*` trait/type/impl。

## 2. 约束裁定与冻结分区

### 2.1 357 条 frozen Task 5 身份

BASE consumer manifest 原始 SHA-256：
`241a2d42b74e27ae2b0e41729d3088e94f464bfe7984699073a20f834ce908bc`。

父级只读审计给出的 frozen 357 行分区被逐身份绑定：

| disposition | 行数 | Task 5 处置 |
| --- | ---: | --- |
| `DRUID_INTERNAL_REFERENCE_CLOSED` | 180 | Task 5 已从旧 source/public owner 删除或迁入 Druid 私有模块 |
| `TASK6_COMPATIBILITY_PENDING` | 69 | Task 6 删除 Physical SPI / wrapper adapter / Druid facade 时处理 |
| `TASK7_RESOURCE_OPERATIONS_PENDING` | 108 | Task 7 以 `ResourceOperations` 完成 canonical resource operation replacement |

Druid 81 行子分区是 61/20：

- Task 5 关闭 61：`PhysicalConnectionInfo=8`、`PreparedStatementKey=40`、
  `PreparedStatementMethodType=13`；
- Task 6 compatibility 20：10 个 family 各 2 行。

去掉 token 坐标后的 Druid 81 行 stable identity SHA-256 为
`cd2ecd715546f1f2da5fe51cbf0f79a47dc73b240252120e8d841aa305ed6b82`。
verifier 从 BASE 自行重建并校验该哈希，不依赖 `/tmp` 文件存在。

### 2.2 当前兼容新增与 shim 转换

Task 5 实施产生 6 条明确的 Task 6 compatibility 新身份：

- `standard_prepared_shell.rs` 精确 4 行：旧 key 的 import/type/call 与旧 MethodType 的
  单一安全宏实参；完整保留 9 个 Java overload，不用 unsafe；
- `physical_prepared_statement_key_type.rs` 1 行：旧 lease trait 方法签名的 type-position macro；
- `physical_connection_info.rs` 1 行：旧 factory result 迁入 Druid 私有阶段对象。

这 6 条 stable identity SHA-256 为
`773722c3231615e5227eea118f046b47c0fa73f7dddfd9cc1486a4edfb2f64c7`。
此外，17 条 frozen Druid compatibility import 已变成本地 facade/shim，精确身份 SHA-256 为
`9d6556397b230e5e76aa8acc006747d24e54fcdd4c79c11ed9cc8246b400f5f9`。

因此当前 checked Task 5 history/current 表为 `363 = 180 + 75 + 108`；其中 `75` 必须解释为
`69 frozen + 6 approved additions`，不能改写 frozen 357 分区。verifier 对 81、17、6 三个
stable multiset 分别锁 hash，遗漏、重复、伪造或仅放入允许目录都会失败。

## 3. TDD RED 证据

### 3.1 首轮缺失行为

| RED gate | 真实失败 |
| --- | --- |
| `single_standard_logical_contract_test` | 16 个目标 compile errors：8 个 canonical reset 方法缺失、6 个 proxy trait impl 缺失、2 个构造入口缺失 |
| `raw_logical_ownership_test` | 13 个目标 compile errors：standard holder/owner/close 路径缺失 |
| Task 5 live verifier | Druid private MethodType 固定文件缺失；holder 仍以 Physical 为主 |
| semantic `logical-live` | CLI phase 不存在，随后第 5 个目标 logical row 仍为 planned |
| standard Prepared cache | 同 SQL prepare 计数 `2`，预期复用后为 `1` |
| native shutdown | raw close 计数 `0`，预期 worker 完成后为 `1` |
| Task 5 Clippy | 8 个 warning；包括转入本任务的 `druid_pooled_connection.rs` 与 `result_set_filter_chain.rs` 两个历史 warning |

### 3.2 实施与自审期间的附加 RED

这些失败均先由可观察断言复现，再修改 production：

| RED | 原因 | GREEN 处置 |
| --- | --- | --- |
| Physical lease prepare 返回 `UnsupportedOperation` | 为关闭旧 key 引用误删两个 delegate override | type-position macro 把签名限制在 Task 6 boundary，完整委托恢复，lease `2/2` |
| native shutdown cached Prepared close `0 != 1` | worker 只拿 raw connection，holder cache 被 Drop | worker 的 standard payload 改为 boxed holder；先逐项 close cache，再 raw close |
| external Prepared close `0 != 1` | external lease 也使用 native cache 容量 | external owner cache 容量为 0，statement 立即 close，绝不进入 native pool |
| stale Prepared 的第二租约 prepare 次数 `1 != 2` | 旧 logical proxy 关闭后仍能写入同 holder cache | lease inactive 时强制 raw statement close，禁止写入下一租约 cache |
| post-close ResultSet `value()` 仍成功 | 只有 `next()` 校验 logical actor | `value/meta_data/was_null/next/is_closed` 统一检查 lease actor |
| post-close Statement `is_closed=false` | trait `is_closed` 只查询 raw handle | Statement/Prepared/Callable logical close state统一包含 lease actor |
| callable overload `M1 != Precall1` | 一次错误的 neutral MethodType 简化 | 安全宏展开 9 个精确映射；source 仍只有一个旧 MethodType stable identity |
| raw close failure可重试 | connection 在 close error 后放回 holder | take-once owner 记录第一次错误但不重新插入；attempt count 保持 1 |

## 4. GREEN 实现

### 4.1 Holder 与 owner policy

- `DruidConnectionHolder` 的主 raw slot 是
  `Option<Box<dyn druid_rdbc::connection::Connection>>`。
- `ConnectionOwnerPolicy::{NativePool, ExternalLease}` 是 Druid 私有枚举；没有新增公开
  Raw/Physical hierarchy。
- Task 6 transitional Physical slot仍保留，但 canonical native/external 入口均创建 standard holder。
- external callback 只取得 raw standard connection；native callback 在该分支永远不执行。
- logical close 通过 `recycle_once` 移交 holder，double close / Drop 不会重复回收。
- eviction/shutdown worker 对 standard target 持有完整 boxed holder，关闭 cached Prepared 后仅调用
  一次 raw `Connection::close`；cached/raw 任一失败都会继续执行剩余 close，并返回第一个错误。

### 4.2 Standard reset/recycle

canonical `Connection` 增加 fail-closed 的以下 JDBC 状态方法：

- `auto_commit / set_auto_commit`；
- `read_only / set_read_only`；
- `catalog / set_catalog`；
- `schema / set_schema`；
- `warnings / clear_warnings`。

默认实现全部返回 `UnsupportedOperation`，不存在 default-success。holder 首次入池捕获 standard
defaults；显式 logical close 按语义执行 transaction rollback、read-only/catalog/schema/auto-commit
恢复与 warnings 清理。任一 rollback/reset/validation 失败转为 discard，错误不会污染 idle queue。

### 4.3 六个直接 canonical proxy impl

以下 Druid production objects 直接实现同一 standard tree：

1. `DruidPooledConnection: Connection`；
2. `DruidPooledStatement: Statement`；
3. `DruidPooledPreparedStatement: PreparedStatement`；
4. `DruidPooledCallableStatement: CallableStatement`；
5. `DruidPooledResultSet: ResultSet`；
6. `DatabaseMetaDataProxyImpl<'static>: DatabaseMetaData`。

实现使用 owned driver handles 与 logical actor token；connection-first close 后 live
Statement/Prepared/Callable/ResultSet/DatabaseMetaData 都拒绝继续工作。ResultSetMetaData 与
ParameterMetaData 是 immutable `Arc` snapshots，连接与 live cursor 关闭后仍可使用。

### 4.4 Prepared cache 与生命周期

- Druid 私有 `StandardPreparedStatementCache` 存储
  `Box<dyn druid_rdbc::statement::PreparedStatement>`，使用 access-order LRU。
- Druid 私有 `PreparedStatementKey/PreparedStatementMethodType` 保留 SQL、catalog、9 个 Java
  overload、ResultSet 属性、generated-key mode、column index/name 的完整 equality。
- 正常 native close 把 statement 放回同一 raw connection cache；LRU eviction 与 pool shutdown
  显式关闭 raw statement。
- external owner 不跨租约缓存。
- 旧 logical lease 的迟到 close 只关闭自己的 raw statement，不得进入下一租约 cache。

### 4.5 Druid 私有一对象一文件

Task 5 在 `crates/druid/src/core` 建立真实私有对象：

- `PhysicalConnectionInfo`；
- `PreparedStatementKey`；
- `PreparedStatementMethodType`；
- `ResourceContext`；
- `ResourceFactory`；
- `ResourceOwner`；
- `ResourceId`；
- `ResourceKind`；
- `ResourceState`；
- `ResourceRuntimeCapabilities`；
- `WrapperProxyHooks`；
- `MysqlMetadataCompatibility`；
- `StandardPreparedStatementCache`。

`compat.rs` 只声明/重导出两个 Task 6 compatibility helpers；没有 alias/facade 冒充上述 13 个
concrete objects，没有 `todo! / unimplemented!`，也没有新增 unsafe 或手写 Send/Sync。

### 4.6 RDBC resource public boundary

- 旧 public `RdbcResourceCapabilities/Context/Factory/Id/Kind/Owner/State` 名称在三 crate
  production source 中为 0。
- Druid owner callback、invalidation、abandon 与 exactly-once release 已迁入 Druid private modules。
- `druid-rdbc` 对外仅保留 canonical `ResourceOperations` 与 `ResourceHandleFactory`；其内部
  `ResourceLifecycle/Identity/Type/State` 是 detached standard handle mechanics，不含 Druid pool owner。
- 旧 `Physical*` 与 compatibility impl 仍保留，等待 Task 6；没有提前删除。

## 5. Inventory 与 verifier

### 5.1 Task 5 verifier

`verify_druid_single_standard.py` 动态检查：

- 所有固定文件与 13 个 concrete objects；
- holder standard slot 与 private owner policy；
- 六个 direct standard impl，拒绝 alias/facade、空 impl、default-success、cfg-disabled identity、
  standard impl 内 hidden Physical core 与 duplicated core；
- logical/raw close ownership与 external/native branch；
- BASE consumer SHA、Druid 81 行 partition hash、17 shim hash、6 additions hash；
- 180 CLOSED source absence、Task 7 old capability absence与 Task 6 compatibility source proof；
- Task 4 frozen manifests未改，并运行当前 RBDC 精确 handoff test。

Self-tests：`13/13 PASS`，含 obsolete Task5 LIVE state 与 fabricated compatibility identity 反例。

### 5.2 Denominator

最终动态 source denominator：

```text
PHYSICAL_ROWS=73
PHYSICAL_SHA256=523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1
ADAPTER_ROWS=57
ADAPTER_SHA256=7ef8ec5cb2d63c0e7d446ba22aacfb0b5d8512e9f50827b6c6f7ab60e2d76353
CONSUMER_ROWS=3599
CONSUMER_SHA256=3daca7f8b2e22ea3aa0db5ad658ac0c49ea64e4ef48e06606f285a9b5aeb2087
```

Denominator self-tests：`41/41 PASS`。checked raw TSV hashes见第 7 节。

### 5.3 Semantic logical-live

精确 9 行 logical LIVE：CallableStatement、Connection、DatabaseMetaData、ParameterMetaData、
PreparedStatement、ResultSet、ResultSetMetaData、Statement、Wrapper。

```text
JAVA_SQL=52
JAVAX_SQL=17
UNKNOWN=0
SEMANTIC_SHA256=5d99b57378649dbe16378f9e3992cfed46058bdb12cf138847006dce430cdbd9
LIVE_SEMANTIC_SHA256=f2e1d929af5c90df3bbeecb6416e4470b820105fd9f23cbbeed3246f95d9ed84
```

Semantic self-tests：`18/18 PASS`。`adapter-live` 与 `logical-live` 都保持 52/17/0；
`adapter-live` 仍归一化到 frozen semantic hash。

## 6. 测试与质量门禁

### 6.1 Task 5 / Druid 行为

| Gate | 结果 |
| --- | --- |
| `single_standard_logical_contract_test` | `4/4 PASS` |
| `raw_logical_ownership_test` | `5/5 PASS` |
| `standard_metadata_ownership_test` | `1/1 PASS` |
| `recycle_semantics_test` | `12/12 PASS` |
| `prepared_statement_cache_test` | `6/6 PASS` |
| `rdbc_external_consumer_test` | `2/2 PASS` |
| `callable_statement_semantics_test` | `4/4 PASS` |
| `physical_connection_lease_test` | `2/2 PASS` |
| Druid private resource owner tests | `2/2 PASS` |
| detached canonical resource tests | `2/2 PASS` |

Filter/stat focused regressions：`filter_adapter_semantics 2/2`、`stat_filter_context 4/4`、
`result_set_filter_semantics 28/28`、`pooled_statement_semantics 7/7`，全部 PASS。

`cargo check --workspace --all-targets --all-features`：PASS，无 warning。

### 6.2 Clippy / docs / format

- `cargo clippy -p druid --all-targets --all-features -- -D warnings`：PASS。
- `cargo clippy -p druid-wrapper --all-targets --all-features -- -D warnings`：PASS。
- `cargo clippy -p druid-rdbc --all-targets --all-features -- -D warnings`：PASS。
- 两个已转入 Task 5 的历史 Druid warning 已最小修复。
- Rust 1.97 同时发现 5 个既有测试 redundant-closure warning；仅机械改为方法引用。
- 三 crate `RUSTDOCFLAGS=-D warnings cargo doc --all-features --no-deps`：PASS。
- doctest：Druid `0`、wrapper `0`、RDBC `1/1 PASS`。
- `cargo fmt --all -- --check`、Black 24.10 `--check`、Python `py_compile`、
  `git diff --check`：PASS。

### 6.3 Task 3 / Task 4 回归

- Task 3 purity self-tests：`103/103 PASS`；transitional live：`STANDARD_FILES=29 PASS`。
- Task 4 coverage/runtime frozen manifests保持原 bytes/hash。
- 旧 Task 4 verifier 按计划只在一个 source hash 处 stale：
  `rbdc_connection_adapter_test.rs` 从 frozen
  `f5e8df13bed46fe23a274dea188cb02d0ff8827e3cb5ba8852e759a02b0edb1e`
  变为当前
  `b3fc8750e3cfaf552096cd3c0db0501f0bc312a52b0ad6b4b9af9e94d9daf96c`。
- 因此旧 Task 4 self-test是 `35/38`，3 个 error 全部是同一预期 stale hash；旧 live verifier
  exit `1`。这两个结果没有伪报 GREEN，也没有重写 frozen manifest。
- Task 5 handoff verifier要求 stale set恰好只有这一项、确认旧/新 resource 名精确替换，并实际执行
  `rbdc_standard_factory_emits_typed_runtime_trace`：`1/1 PASS`。

外部真实 RBDC/HTTP/libSQL/JDBC Agent 服务继续诚实记录为 `NOT_RUN`；本任务没有声称外部服务验证。

## 7. 最终 checked artifact hashes

```text
rdbc-production-consumers.tsv
  9a602b4cf118648767e68e48f45003bb0847fe8a363f2a313fe4c1426a4b1ce7
rdbc-adapter-implementations.tsv
  c2af3627c426a2badfaf24a6447e4d53dd3c0d15cc76f8536a546cb758f8467e
jdbc42-semantic-inventory.tsv
  f2e1d929af5c90df3bbeecb6416e4470b820105fd9f23cbbeed3246f95d9ed84
rdbc-adapter-coverage-regions.json (Task 4 frozen)
  f7b3d4b429fe9e5c4bfe954082f0518d8e78635f6f7cc21bde1ffcd40f2307c9
rdbc-adapter-runtime-traces.tsv (Task 4 frozen)
  106323e17904d377172986ce7464a54de60b4bb575e5f67e333e37f16188a3ec
```

## 8. Self-review 与剩余边界

Self-review检查了 holder/raw owner、cache eviction、external lease、reset failure、stale handle、
connection-first close、metadata snapshots、resource owner并发、compatibility identity 与 verifier
fail-closed路径；上述附加 RED 均已关闭。没有发现未处置的 Task 5 correctness blocker。

剩余边界均为已计划 handoff，而不是 Task 5 LIVE：

1. Task 6：69 条 frozen compatibility + 6 条本任务新增 compatibility，合计 75 条；届时原子删除
   Physical SPI/adapters/Druid facade与两个 compatibility helper，并更新对应 public/hash evidence。
2. Task 7：108 条 `TASK7_RESOURCE_OPERATIONS_PENDING`；以 `ResourceOperations` 完成 resource
   capability/operation replacement。
3. Task 4 的旧 verifier在获准 RBDC source rename 后预期 stale；Task 5 handoff是当前有效证据，
   frozen Task 4 artifacts未被改写。
4. 外部真实服务未运行；确定性本地 factory/adapter证据保留，不能替代真实部署验证。

## 9. 提交状态

本报告与 production/tests/verifiers/manifests将作为一个独立 Task 5 commit 精确暂存；不 push。
commit SHA 在提交完成后的 handoff 中报告，避免通过 amend 或第二个仅写 SHA 的提交改写历史。

## 10. Independent review correction — FIX_REQUIRED（append-only）

独立 spec + code review 针对 `d1ccc8e..cf15451` 给出 **FIX_REQUIRED：3 Critical、
6 Important、0 Minor**。本节覆盖此前第 8 节“没有 Task 5 blocker”以及 ledger 中
“implementation complete”的结论；这些旧文字保留为历史记录，不再代表当前状态。

开放 finding：

1. C1：`DruidPool::new_standard` admission 无条件读取扩展 state getter，七个真实 Task 4
   adapter 均会在入池前失败；需要显式 managed/unsupported state capture 与真实 adapter matrix。
2. C2：borrow/return/discard/validation/wrap 仍硬编码 Physical slot，standard holder 无法进入
   第二次 native checkout，也没有使用实际 `test_on_return`。
3. C3：canonical Connection/Statement/Prepared/Callable/ResultSet 绕过现有 Filter/Wall/Stat/
   ExceptionSorter/after-event core。
4. I1：live metadata `tables()` 返回 raw ResultSet，cursor 没有继承同一 logical lease generation。
5. I2：standard Prepared cache 忽略 `pool_prepared_statements=false/zero`。
6. I3：standard handles 未加入 logical close cascade，且 Prepared fallible close 在 cleanup 前
   `take()` raw handle，失败会丢失 terminal ownership。
7. I4：Druid `ResourceContext::release_with` 未在观察 `Releasing` 前 pin + enable Notify waiter，
   存在 lost wakeup。
8. I5：新 Druid key/method-type modules/types/methods仍从外部可达，形成第二棵 public tree。
9. I6：`verify_task4_evidence_handoff` 仅定义未调用，Task 5 live gate没有执行 frozen handoff。

当前 Task 5 状态改为 **FIX_ROUND_1_IN_PROGRESS**；Task 6 明确阻塞。修复范围仅限上述九项，
必须逐 finding 留下真实 RED→最小 GREEN，并保持 frozen classifier `180/75/108`、logical LIVE
9 行和 semantic `52/17/0`。完成实现与全门禁后仍需新的独立 review 才能解除 Task 6 阻塞。

## 11. Fix round 1/5 implementation — pending independent review

本轮从 `cf15451a17dd24729b099fd3ad4caf29e81fb43f` 开始，只处理独立 review 的
3 Critical + 6 Important。当前状态为 **FIX_ROUND_1_IMPLEMENTED_PENDING_REVIEW**；这不是
REVIEW CLEAR，Task 6 继续阻塞。

### 11.1 C1 — truthful standard admission/reset

- `StandardConnectionCompatibility` 与 `ConnectionDefaults` 将 auto-commit、read-only、catalog、
  schema、warnings 分别保存为“可观察值”或 typed Unsupported；不再把未观察状态伪造成
  `true/false/None` capability。
- 只有 `UnsupportedOperation` / `SQLFeatureNotSupported` 被视为 unmanaged；其他 getter 错误原样
  终止 admission。reset 只读取/设置首次确认为 supported 的字段。
- rollback 只在 transactions capability 为真且 auto-commit 被真实观察为 `false` 时执行。
- legacy Physical 初始化顺序保持不变；capability skip 仅作用于私有
  `StandardConnectionCompatibility`，避免把 Task 6 过渡 SPI 的旧 setter 行为误删。

RED/GREEN：

- 真实 SQLx factory 经 `DruidPool::new_standard` 最初以
  `UnsupportedOperation(connection_read_only)` 获取超时；GREEN 后 SQLx/Toasty/RBDC default
  admission `3/3 PASS`，all-features 加 DuckDB 为 `4/4 PASS`。
- 全 Unsupported fixture 最初以 `connection_auto_commit` 获取超时；GREEN 后 borrow/return 成功且
  五类 setter/clear-warning attempt 总数为 0。
- 注入 `auto_commit_getter` 的非 Unsupported driver error 现在确定性终止 holder admission。
- HTTP SQL、libSQL、JDBC Agent 没有新增 public fake seam；由未改的 Task 4 deterministic
  factory/runtime/coverage evidence 与本轮 Druid admission contract 组合证明。真实外部服务仍
  `NOT_RUN`。

### 11.2 C2 — standard holder native lifecycle

- idle borrow、return、destroy、wrap、test-on-return、keep-alive 都按 holder 实际
  `standard_connection | physical_connection` slot 分支。
- standard native return 使用 standard recycle constructor 与真实 `test_on_return`；ExternalLease
  只执行 external callback，永不进入 native idle queue。
- second checkout、validation discard、idle expiry、shutdown、Java-compatible maxIdle 都以 raw
  create/close/validate 精确计数验证。

Fresh RED：borrow-close-borrow 的 factory create count 为 `2` 而非 `1`。GREEN 后同一 holder
复用，create count `1`；invalid/expired/shutdown holder 只销毁一次。

### 11.3 C3 — one Filter/Stat/error policy

- 新私有一对象文件 `druid_execution_filter_policy.rs` 提供唯一
  `before -> effective SQL -> raw -> after` core。canonical standard policy 与 transitional
  Physical update/generic execute/prepared typed-parameter families都适配该 core，不互相委托。
- canonical Statement/Prepared/Callable query/update/execute 使用同一 policy；generic execute
  中的 ResultSet 也包装为 `DruidPooledResultSet`，进入同一 registry/lease。
- ExceptionSorter 对 typed SQL error 执行 fatal classification，并把 logical disposition 标记为
  discard；after-error 仍恰好执行一次。
- standard commit/rollback/setters/validation/clear-warnings补齐 connection before/after event；
  logical policy跟踪可观察 auto-commit与 transaction start/commit/rollback stats。
- async ResultSet chain保持 crate-private；现有 public `ResultSetFilter` 只新增 primitive
  before/after observation hooks，不暴露第二棵 proxy 类型树。Stat/Wall/Log 与自定义 recording
  filter 的 open/next/close 均走一次；Log 的 Physical/standard 分支共享同一 logging core。
- transitional `prepare_physical_statement/prepare_physical_call` 补回 sorter delegation，修复 full
  regression发现的结构化 SQL error bypass。

Fresh RED：generic execute raw ResultSet 的 Stat `open_count` 为 `0`（期望 `1`），且能跨 lease；
GREEN 后 C3 suite `5/5 PASS`，证明 SQL rewrite、before/after、Wall raw-call rejection、三种 statement
family、open/next/close/fetch stats、fatal discard、commit/rollback stats 与 result hooks精确一次。

### 11.4 I1–I3 — metadata/cache/resource ownership

- I1：`DatabaseMetaData::tables` 返回同 lease/generation 的 `DruidPooledResultSet`。第一 lease
  close 并 reborrow holder 后，旧 metadata cursor 的 next/value/meta_data 全部拒绝；先前取得的
  ResultSetMetaData/ParameterMetaData snapshots 继续可用。
- I2：standard cache capacity 由 `pool_prepared_statements && capacity > 0` 决定；disabled、zero、
  enabled、external matrix均验证 prepare/raw-close次数。
- I3：holder-owned `StandardResourceRegistry` 对 Statement/Prepared/Callable/ResultSet 逆序异步
  cascade。Prepared 在 clear/is_closed/raw close 成功前不 `take` handle；失败保留并可重试。
  eviction close失败的 raw handle保存在 cache pending-close owner中，shutdown确定性重试。

Fresh RED 包括：disabled cache prepare count `1`（期望 `2`）；connection-first cascade raw close
计数均为 0；Prepared cleanup failure后第二次 close 返回 discarded；并发 connection/statement
close raw attempts `2`（期望 `1`）；eviction failure shutdown attempts `2`（期望 `3`）。上述均 GREEN，
并增加 drop-without-close、double/concurrent close 与 stale close 不跨 lease cache 的证明。

### 11.5 I4 — Notify race ruling

生产代码已镜像 detached lifecycle 的模式：先创建 `notified()`，`pin!` 并 `enable()`，再观察
`Releasing`。确定性 barrier/timeout测试 PASS。

必须诚实记录：review描述的旧代码 lost-wakeup 在当前 Tokio 1.53 `notify_waiters` generation
语义下未能复现；测试在生产修复前也是 PASS。因此本项不是伪造 RED，而是防御性对齐与永久回归
门禁。若独立 reviewer要求证明不同 Tokio/`notify_one` 语义，需另行 ruling，不能在报告中冒充。

### 11.6 I5 — private Druid types

- 新 key/method/resource/owner/policy/registry/async-chain modules与类型均为 `mod` + `pub(crate)` 或更严；
  删除 public `key()` 暴露。只保留 Task 6 frozen compatibility facade。
- 原外部 cache/key 测试迁入 crate-private unit suite（`6/6 PASS`）；wrapper/callable测试改用公开对象
  identity、captured compatibility key 与 cache count，不再穿透私有对象。
- external consumer compile-fail最初意外编译成功；GREEN 后 private module paths均拒绝，公开
  `druid::sql::*` contract仍编译通过。

### 11.7 I6 — live handoff and runtime composition

- `verify_repository` 现在必调 `verify_task4_evidence_handoff` 并传播失败。
- handoff严格检查两个 frozen artifact hashes、全 requirement source hashes、唯一 RBDC stale tuple、
  exact resource rename，再运行 Unsupported admission、all-feature adapter admission 与当前 RBDC
  exact trace。
- Task5 live还运行 pool/cache/cleanup、Filter/Stat/error、metadata cross-lease 与 raw/logical owner
  suites；shared execution core与四个 admission identity有静态 + runtime双重证明。
- verifier self-tests从 `13/13` 增至 `19/19`，新增 coverage hash、runtime hash、stale tuple、exact
  RBDC command failure、admission omission、shared-core omission与runtime failure propagation反例。

Fresh RED：monkeypatch `verify_task4_evidence_handoff` 为 sentinel error时，旧
`verify_repository` 没有抛错；GREEN 后 sentinel与所有新增 mutation均 fail closed。

### 11.8 Final inventory and frozen handoff

Task5 classifier保持：

```text
TASK5_CONSUMER_ROWS=363
DRUID_INTERNAL_REFERENCE_CLOSED=180
TASK6_COMPATIBILITY_PENDING=75
TASK7_RESOURCE_OPERATIONS_PENDING=108
DRUID_STANDARD_IMPLS=6
DRUID_INTERNAL_OBJECTS=13
```

logical-live保持精确 9 行与 `JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0`；semantic/live hash分别为
`5d99b573...cdbd9` / `f2e1d929...d9ed84`。

Fresh denominator为 `PHYSICAL=73 / ADAPTER=57 / CONSUMER=3629`，consumer hash
`3857923c2a618abea89c4bc07d7a0ffc477dfe97d51a4b66bb822b3d3f79b84e`。变化来自本轮真实
runtime tests/hooks与 shared-core source identities；self-test `41/41 PASS`，Task5 frozen
180/75/108 identity hash未改变。最终 checked `rdbc-production-consumers.tsv` SHA-256 为
`3def7e2e8348db2e39f7dde270645530b591518e01d562ed5d8c99d613c0bf0d`。

Task 4 frozen artifacts字节未改：coverage
`f7b3d4b...07c9`，runtime `106323e...a3ec`。旧 Task4 self/live继续诚实为 `35/38` / exit 1，
且只因 RBDC evidence source从 frozen `f5e8df...b1e` 变为批准的 current `b3fc87...f96c`。
Task5 exact handoff PASS；当前 SQLx default与HTTP SQL all-features隔离 compiler coverage代表样本
均 PASS，每族 15 个 exact compiler functions。

### 11.9 Verification summary

- focused：raw/logical `5/5`、privacy external `3/3`、standard logical `18/18`、C3 `5/5`、
  metadata `1/1`、Druid lib `52/52`；all-feature admission `4/4`。
- full：Druid、druid-wrapper、druid-rdbc 的 all-target/all-feature test全部 PASS。
- workspace `cargo check --workspace --all-targets --all-features` PASS。
- scoped Clippy：Druid、wrapper、RDBC all-target/all-feature `-D warnings` PASS；两处原 Task5 warning
  与本轮新增 lint均清零。
- 三 crate rustdoc `-D warnings` PASS；doctest `0 / 0 / 1 PASS`。
- Task3 purity `103/103` + transitional `STANDARD_FILES=29` PASS。
- Task5 verifier `19/19` self + live PASS；semantic `18/18` + logical-live PASS；denominator
  `41/41` + live PASS。
- fmt、Black 24.10、Python compile、diff/stage检查在最终提交前再次执行。

外部真实 RBDC/HTTP/libSQL/JDBC Agent服务仍为 `NOT_RUN`。本轮没有安装工具、修改 Cargo topology、
切分支、创建 worktree、amend 或 push。

### 11.10 Remaining risk and status

没有已知未处置的 round1 production correctness finding，但只有独立 re-review可以确认
3C+6I为 ADDRESSED。I4 的旧行为不可复现结论需要 reviewer接受；Task4旧 verifier stale仍按计划由
Task5 exact handoff承接。Task 6在新的独立 REVIEW CLEAR 前继续阻塞。

## 12. Fix round 1 independent re-review correction — FIX_REQUIRED

独立 re-review 针对 `cf15451..314115b` 仍给出 **FIX_REQUIRED**。本节覆盖第 11 节
“没有已知未处置 finding”的结论；历史文字继续保留，但不再代表当前状态。

- C1、C2、I1、I2、I3、I4、I5：`ADDRESSED`，round 2 必须保持。
- C3：`PARTIAL`。canonical create/prepare/call/metadata、prepared typed parameter snapshot、
  ResultSet proxy-aware open 与 value/meta_data/was_null filter paths仍开放。
- N1：新 Important。registry close snapshot未 seal admission，late ResultSet可在 drain 后注册；
  显式关闭也不移除 strong entry，长 lease会累积历史 handle。
- I6：`PARTIAL`。live verifier未绑定上述 C3/N1 residual，RBDC handoff只运行普通 exact test，
  未执行当前 compiler coverage v2 的 15-function gate。

当前状态改为 **FIX_ROUND_2_IN_PROGRESS**。本轮范围仅 C3、N1、I6；Task 6继续阻塞。
必须保持 frozen `180/75/108`、logical 9、semantic `52/17/0`、Task4 frozen artifacts以及
已 addressed round1 行为。

## 13. Fix round 2/5 implementation report — pending independent re-review

本轮从 `314115b` 开始，仅处置 Round 1 re-review 的 C3、N1、I6；C1/C2/I1-I5 的既有行为保持。
没有创建 worktree/分支、没有 reset/amend/push，没有修改 Cargo topology、Task 4 frozen artifacts
或 `druid-admin`。

### 13.1 Fresh RED

- C3 prepare/parameter：prepared batch 返回 `UnsupportedOperation(statement_add_batch)`；setter
  失败后 Filter 参数快照为 `[]` 而不是已成功的 `Int(10)`；Wall prepare denial仍调用 raw；
  tenant ResultSet callback为 `0` 而不是 `1`。
- C3 configured setup自审追加RED：为canonical tracking raw实现query-timeout override时出现6个
  `E0407`，暴露standard Statement没有该接口；此前`query_timeout=13`只写入Druid的Physical占位
  statement，未触达root/prepared/cached/callable raw对象。
- C3 ResultSet getter协议：新增 value/meta_data/was_null Filter override时出现 6 个 `E0407`，证明
  canonical hook不存在；proxy-aware open拒绝后 raw ResultSet close count为 `0`，期望 `1`。
- N1 registry三个确定性测试最初 `3/3 FAIL`：Closing/Closed后的 late register未关闭 raw、128 个
  显式关闭资源的 `Weak` 仍可 upgrade、第二个 concurrent `close_all` 在 winner drain前返回。
- I6 verifier自测最初 `23` 个中 `2 failure + 3 error`：缺少 exact-test runner、11 个 C3/N1
  identity未执行、current RBDC compiler coverage入口未调用。

### 13.2 C3 shared Filter/Wall/Stat/error policy GREEN

- `DruidExecutionFilterPolicy` 现在由 Physical与canonical共同使用 prepare SQL链、statement
  create/prepare/call after事件与计数、普通/Prepared batch around-chain。effective SQL同时进入 raw
  prepare与 `PreparedStatementKey`；query timeout、structured error sorter与 fatal discard保持一致。
- canonical `Statement`新增typed query-timeout getter/setter，默认明确返回Unsupported；Druid三类proxy
  直接调用其真实canonical raw handle。configured测试证明root/prepared/cache-hit/callable四次setter
  均收到`13`，getter也返回`13`，不再经过Physical占位对象。
- Prepared/Callable参数槽、clear与batch snapshot只在 raw setter/clear/add成功后更新。执行 Filter
  接收有序 typed `PreparedInputParameter`与scalar params；`prepared_parameters=Some(...)` 使 Wall在
  执行期只复核、不重复 tenant rewrite，raw对象使用prepare边界的effective SQL。
- canonical metadata进入同一 connection metadata permission/after/error链；Wall rejection在raw调用前
  终止，raw create/prepare/call/metadata SQL错误均进入 sorter。
- standard ResultSet使用proxy-aware open；Wall从immutable standard metadata生成hidden/tenant映射。
  next/close/value/meta_data/was_null进入同一 `ResultSetFilter`注册顺序并分类错误。open/meta预处理失败时
  注册前raw cursor异步关闭恰好一次，不遗失owner。
- focused C3最终 `12/12 PASS`；旧Physical回归 `filter_adapter 2/2`、`stat_filter_context 4/4`、
  `result_set_filter_semantics 28/28`、`pooled_statement_semantics 7/7` 全部PASS。

### 13.3 N1 sealed generation registry GREEN

- registry在一个Mutex下维护 `Open | Closing | Closed`、lease generation及id→Arc entries。
  register在Open且generation匹配时先在同一临界区绑定generation/id token再入表；Closing/Closed或
  generation不匹配时不暴露handle，在锁外立即async close并返回typed discarded error。
- `close_all` 使用先创建并 `pin + enable` 的Notify future，再检查Closing；一个winner drain，其他
  caller等待同一完成通知。失败close保留retry ownership；成功显式close按generation/id移除strong
  entry，避免长lease历史积累。
- metadata竞态测试的barrier位于raw ResultSet已经返回、proxy-aware open已进入、registry register尚未
  发生的精确位置。旧lease close/seal后先reborrow holder，再释放barrier；late register关闭raw一次、
  返回失败，next lease仍可执行，cursor不跨lease。
- registry `3/3 PASS`，metadata ownership `2/2 PASS`，Druid lib在all-feature下 `55/55 PASS`。

### 13.4 I6 exact runtime and Task 4 coverage handoff

- Task5 live verifier除既有四个suite外，逐个运行并解析11个exact C3/N1 tests；真实Rust函数定义必须
  唯一，Cargo即使exit 0但报告`running 0 tests`也失败。self-test最终 `23/23 PASS`，comment-only、
  missing identity、zero-test与coverage runner failure mutation均fail closed。
- handoff保留Task4 frozen coverage/runtime bytes，只把唯一批准RBDC test source tuple
  `f5e8df...b1e -> b3fc87...f96c`装配成当前coverage contract；随后调用原Task4隔离runner和exact
  validator，RBDC 15个compiler functions全部PASS。当前SQLx default与HTTP_SQL all-features也分别
  `15/15 PASS`。
- 旧Task4 verifier仍诚实为self `35/38`、live exit `1`，三个self error/live error全部且仅因上述
  frozen RBDC source hash。没有一般化hash bypass，也未改写frozen artifact。

### 13.5 Final inventories and hashes

```text
TASK5_CONSUMER_ROWS=363
TASK5_CONSUMER_CLOSED=180
TASK6_COMPATIBILITY_PENDING=75
TASK7_RESOURCE_OPERATIONS_PENDING=108
DRUID_STANDARD_IMPLS=6
DRUID_INTERNAL_OBJECTS=13

PHYSICAL_ROWS=73
ADAPTER_ROWS=57
CONSUMER_ROWS=3643
CONSUMER_SHA256=2304c766d9855e7d254020bf343fda9709b222d666b25e103c491dcfa4ab4e97
```

`rdbc-production-consumers.tsv`按当前production source坐标重新emit，Task5 frozen identities/states与
6 additions未变；checked SHA-256为
`c73fa6db70caab834aa8d64a8a83b86475b1fb3d9cf7a82f36c417690a26ad13`。

logical-live继续精确9行：`JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0`；semantic/live hash为
`5d99b573...cdbd9 / f2e1d929...d9ed84`。Task4 frozen coverage/runtime SHA-256仍为
`f7b3d4b...07c9 / 106323e...a3ec`。

### 13.6 Full gates and remaining boundary

- Task5 self/live：`23/23 PASS`；live `363/180/75/108 + 6 impl + 13 object`。
- semantic self/logical-live：`18/18 PASS + 52/17/0`；denominator `41/41 PASS + 73/57/3643`；
  Task3 purity `103/103 PASS + STANDARD_FILES=29`。
- focused：C3 `12/12`、metadata `2/2`、registry `3/3`、standard logical `18/18`、raw owner
  `5/5`、recycle `12/12`、external/privacy `3/3`、admission `4/4`及既有Physical Filter/Stat均PASS。
- `druid`、`druid-wrapper`、`druid-rdbc` all-target/all-feature tests全部PASS；workspace
  all-target/all-feature check PASS。
- 三crate scoped Clippy `-D warnings`、rustdoc `-D warnings`、doctest `0/0/1`、rustfmt、Black、
  Python compile与diff-check全部PASS。

一次在llvm-cov后并行启动四个共享target Cargo进程时出现缺失中间`.rmeta/.rlib`的环境性失败；改为
顺序重建后所有相同测试与全量门禁通过，不作为代码GREEN证据。

真实外部RBDC/HTTP/libSQL/JDBC Agent服务仍为`NOT_RUN`；deterministic local生产路径与compiler
coverage不冒充外部部署证据。当前状态是 **FIX_ROUND_2_IMPLEMENTED_PENDING_REVIEW**；只有独立
re-review可将C3/N1/I6判为ADDRESSED，Task 6继续阻塞。

## 14. Fix round 2 independent re-review correction — FIX_REQUIRED

独立re-review针对`314115b..b1dd9a7`仍给出 **FIX_REQUIRED**。本节覆盖第13节的
`FIX_ROUND_2_IMPLEMENTED_PENDING_REVIEW`完成倾向；历史文字保留为当轮证据，但不代表当前状态。

- C3：`PARTIAL`。canonical Prepared/Callable generic `execute_prepared`仍直接调用raw，绕过typed
  parameter Filter/Stat/sorter并泄漏raw ResultSet；ordinary canonical Statement batch仍绕过add边界
  rewrite/deny及shared batch around policy。
- N1：`PARTIAL`。registry仅在register时封闭；raw acquisition到register之间没有pending permit，
  holder可先回池/reopen，late close failure还可能进入新generation。
- I6：`PARTIAL`。exact runner本身fail-closed，但method obligations与exact test list未覆盖上述C3/N1
  residual，删除相关policy仍不会使live verifier失败。
- C1/C2/I1-I5：re-review未发现回归，继续`ADDRESSED`。

当前状态改为 **FIX_ROUND_3_IN_PROGRESS**。本轮仅拥有C3 residual、N1 pre-raw pending admission与
I6 evidence扩展；frozen `180/75/108`、6 additions、logical 9、semantic `52/17/0`、Task4 frozen
artifacts以及已处置行为必须保持。Task 6继续阻塞。

## 15. Fix round 3/5 implementation report — pending independent re-review

本轮从`b1dd9a7`开始，只处置Round 2 re-review留下的C3 generic execute/ordinary batch、N1
pre-raw pending admission和I6 evidence缺口。C1/C2/I1-I5既有行为保持；没有创建worktree或分支，
没有reset/amend/push，没有修改Cargo topology、Task 4 frozen artifacts或`druid-admin`。

### 15.1 Fresh RED

- C3 focused初始为`12 PASS / 4 FAIL`：Prepared generic execute、Callable generic execute的Filter
  计数均为`0`而不是`1`；ordinary Statement batch没有before/after事件；Wall拒绝的batch entry仍到达raw。
- N1把pre-raw permit契约写入producer/registry测试后出现`8 x E0599`，所有失败均是生产registry尚无
  `begin_admission`，证明旧实现只在raw acquisition之后register时才seal。
- I6 mutation/self-test初始出现`1 failure + 2 errors`：generic execute/batch method obligation、pending
  producer覆盖和新race测试未被live verifier绑定。

这些RED均在production修改之前取得；最终GREEN没有通过删除、ignore、cfg-disable或放宽断言获得。

### 15.2 C3 generic execute与ordinary batch shared policy GREEN

- `DruidPooledPreparedStatement::execute_prepared`与Callable继承实现都进入唯一的
  `execute_standard_prepared_results`。该core使用prepare边界的effective SQL、当前有序typed parameter
  snapshot和同一个before/raw/sorter/after/stat policy；不会在execute边界再次做tenant rewrite。
- generic `ExecutionResult::ResultSet`逐个包装、注册为同lease/generation的
  `DruidPooledResultSet`；UpdateCount保持原值和顺序。fatal raw error只分类一次、after-error只触发一次并
  将logical disposition标记为discard。
- ordinary canonical Statement的`add_batch`先经共享`statement_add_batch_sql`做Wall rewrite/deny；仅raw
  add成功后才保存effective SQL。`clear_batch`同步清理raw与logical顺序状态，`execute_batch`与transitional
  Physical共同使用`execute_batch` around core，保留partial counts、Stat与ExceptionSorter语义。
- 新增5个精确行为测试覆盖Prepared/Callable generic ResultSet、typed params、fatal error以及ordinary batch
  rewrite/deny/order/clear/partial fatal；C3 focused最终`17/17 PASS`。transitional Physical
  `pooled_statement_semantics 7/7`、filter adapter `2/2`、stat context `4/4`、ResultSet filter `28/28`、
  Callable `4/4`继续PASS，未形成standard↔Physical delegation或第二份policy body。

### 15.3 N1 generation-scoped pending admission GREEN

- 新私有一对象文件`standard_resource_admission.rs`保存`Weak<StandardResourceRegistry>`与generation；
  `begin_admission`在registry同一把Mutex下只允许`Open/current`并递增pending，RAII Drop递减并notify。
- `close_all`原子`Open→Closing`后先逆序drain已注册资源，再以`pin + enable`的Notify等待pending归零；
  在此之前不能进入Closed、不能reopen，也不能让holder回native idle queue。
- `register(&permit, resource)`只接受同registry、同Open generation的permit。Closing/Closed/wrong-generation
  会在锁外关闭新raw handle并返回typed error；关闭失败的handle与error保留在旧generation，下一generation
  不可打开，后续`close_all`确定性重试。successful explicit close仍按generation/id unregister，避免长lease
  强引用累积。
- permit在任何可能产生live resource的raw操作之前取得：Connection create/prepare/call；root Statement
  query/generic execute；Prepared query/prepared query/generic execute；Callable query/prepared query/generic
  execute；metadata tables。ResultSet/Prepared/Callable constructors要求传入permit，调用者不能漏传。
- registry最终`7/7 PASS`：cancellation、multiple pending、late close completion、late close failure/retry、
  long-loop unregister和concurrent close_all。metadata cross-lease `3/3 PASS`：connection close在late raw I/O/
  reject-close完成前保持阻塞；close failure使holder discard，shutdown第二次恰好重试；下一lease不接收旧cursor。
  standard logical lifecycle继续`18/18 PASS`。

### 15.4 I6 fail-closed evidence GREEN

- `FIXED_FILES`加入permit文件；canonical method obligation现在要求root Statement batch、Prepared generic
  execute及Callable继承方法，并解析无注释方法体，拒绝direct raw fallback。
- pending policy检查registry state/RAII markers以及每个producer source的精确`begin_admission`/
  `register(admission)`数量；删掉任何一个入口、只留comment或改成0-test都会失败。
- Task5 exact runtime从11扩至21：C3新增5个、registry pending新增4个、metadata late-close-failure新增1个；
  verifier验证真实Rust函数唯一身份、Cargo target和非零executed test count。
- verifier self-test最终`25/25 PASS`；Task5 live执行21个exact tests与当前RBDC coverage-v2的15个exact
  compiler functions，并保持：

```text
TASK5_CONSUMER_ROWS=363
TASK5_CONSUMER_CLOSED=180
TASK6_COMPATIBILITY_PENDING=75
TASK7_RESOURCE_OPERATIONS_PENDING=108
DRUID_STANDARD_IMPLS=6
DRUID_INTERNAL_OBJECTS=13
```

### 15.5 Inventory、Task 4 handoff与完整门禁

- denominator self/live：`41/41 PASS`，`PHYSICAL=73 / ADAPTER=57 / CONSUMER=3647`，consumer hash
  `2cc780035bd025ba47490a4e56ed8ecceac0fa01215d1af0030a7c0bcf4bbec5`；重新emit后的checked
  `rdbc-production-consumers.tsv` SHA-256为
  `24c02233b102d447d51e62a04d6b596079d6d9303d180efbf62908412e3195a6`。
- Task3 purity `103/103 PASS + STANDARD_FILES=29`。semantic `18/18 PASS`；logical-live仍为
  `JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0`，semantic/live hashes保持
  `5d99b573...cdbd9 / f2e1d929...d9ed84`。
- Task4 frozen coverage/runtime bytes未改，SHA-256保持
  `f7b3d4b...07c9 / 106323e...a3ec`。旧Task4 self/live仍诚实为`35/38`与exit `1`，三个self error及
  live error全部且仅是批准的RBDC source hash
  `f5e8df...b1e → b3fc87...f96c`；Task5 handoff运行current RBDC coverage-v2并PASS。SQLx default与
  HTTP_SQL all-features代表覆盖也各自`15/15 PASS`，没有一般化stale bypass或frozen artifact重写。
- focused：C3 `17/17`、registry `7/7`、metadata `3/3`、standard logical `18/18`、raw owner
  `5/5`、recycle `12/12`、external/privacy `3/3`以及既有Physical Filter/Stat/ResultSet/Callable
  回归全部PASS。
- full：`druid`、`druid-wrapper`、`druid-rdbc` all-target/all-feature tests全部PASS；workspace
  all-target/all-feature check PASS；三crate scoped Clippy all-target/all-feature `-D warnings`、rustdoc、
  doctest `0/0/1`、rustfmt、Black、Python compile与diff-check全部PASS。

真实外部RBDC/HTTP/libSQL/JDBC Agent服务仍为`NOT_RUN`；deterministic local路径与compiler coverage
不冒充外部部署证据。当前状态是 **FIX_ROUND_3_IMPLEMENTED_PENDING_REVIEW**；只有独立re-review可把
C3/N1/I6判为ADDRESSED，Task 6继续阻塞。

## 16. Fix round 4/5 implementation report — pending independent re-review

本轮从`d95ef452d496e0e9025c93d67b90c81bae7c101a`开始，只处置Round 3 re-review的两个
Important finding（multi-result cleanup ownership、registry cleanup cancellation safety）与I6证据缺口。
C1/C2/I1-I5既有行为保持；没有进入Task 6，没有修改Cargo topology、Task 4 frozen artifacts或
`druid-admin`，没有worktree/branch/reset/amend/push。

### 16.1 Fresh RED与multi-result staged ownership

- Prepared、Callable和ordinary Statement各自返回三个raw ResultSet，stateful proxy-open Filter拒绝第二个；
  旧实现三个测试都只在返回错误时观察到`1/3` raw close，证明第一个仍在registry、第二个被直接关闭、
  第三个以plain `Box<dyn ResultSet>`丢失async-close ownership。
- 所有raw ResultSet现在先同步转换成`StandardResultSetResource` actor，并在一个registry临界区执行
  batch registration；只有全部raw取得owner后才按原顺序运行metadata/proxy-open。第二个open失败时，
  registry queue关闭并unregister整批，Prepared/Callable/Statement均为`3/3` close exactly once；
  caller仍收到最初的`WallViolation`。
- staged actor区分`opened`与未open cleanup：只有成功完成open hook的ResultSet计入open/close Filter/Stat；
  三raw cleanup测试为raw close `3`、Stat open/close `1/1`。cleanup close失败时原proxy错误仍为primary，
  失败actor保留在旧generation，logical close按既有合同第4次attempt重试后达到`3/3`成功关闭。
- connection-first metadata barrier继续覆盖stage/open竞态：normal close返回`ConnectionDiscarded`，close failure
  返回原`Other`并保持旧generation retry；metadata `3/3 PASS`。

### 16.2 Registry-owned coordinator与cancellation safety

- `Open/Closing/Closed`、generation、pending、resource entry、cleanup waiter、retry和first close error仍由同一
  short critical-section `Mutex`拥有；entry状态为`Open/Queued/Closing/Retry`，没有guard跨`.await`或调用
  resource/filter callback。
- `close_all`只原子seal generation、启动/复用一个registry coordinator并等待Notify；resource从未移入
  caller Future local。abort首个`close_all`时，coordinator在已成功关闭一个resource后继续等待并关闭阻塞
  resource，第二个caller有界完成，两个raw各一次。
- Closing/Closed registration先把actor与oneshot waiter转入registry queue，再idempotently finish admission；
  abort register caller不会丢raw，close coordinator等待pending归零且队列drain完成。先前复用coordinator但
  enqueue未notify造成的lost wake已由每次ownership transfer后的notify修复。
- close failure只进入旧generation `Retry`，同一coordinator run不自旋；下一次`close_all`才重新queue，
  error/discard阻止reopen。multiple callers、permit cancellation、next-lease barrier与128次long-lease
  unregister的retained count zero均保持。
- coordinator task只持`Weak<Registry>`；独立atomic+Notify shutdown signal使holder/registry drop能取消
  正阻塞于driver close的task并释放resource Weak，runtime取消时Drop guard把in-flight恢复为Queued供后续
  caller接管。registry focused最终`10/10 PASS`。

### 16.3 I6 fail-closed evidence

- pending-admission verifier不再只数token：逐一解析11个producer method/helper body，要求
  `begin_admission -> raw acquisition -> registry owner`精确词法顺序；把create-statement permit移动到raw
  acquisition之后的mutation必须以`pre-raw admission`失败，缺少producer仍fail closed。
- staged ownership要求`new_standard_results`、`register_batch`和admission-aware registry cleanup同时存在；
  method delegation仍拒绝Prepared/Callable direct raw fallback。
- Task5 exact runtime从21扩为28：三条multi-result full cleanup、一条cleanup-failure primary/retry、
  aborted close leader、cancelled rejected registration和registry-drop coordinator shutdown；每条仍要求唯一
  Rust definition与exact Cargo `1 passed`。verifier self最终`26/26 PASS`，Task5 live PASS并保持：

```text
TASK5_CONSUMER_ROWS=363
TASK5_CONSUMER_CLOSED=180
TASK6_COMPATIBILITY_PENDING=75
TASK7_RESOURCE_OPERATIONS_PENDING=108
DRUID_STANDARD_IMPLS=6
DRUID_INTERNAL_OBJECTS=13
```

### 16.4 Final inventories and verification

- denominator self/live：`41/41 PASS`，`PHYSICAL=73 / ADAPTER=57 / CONSUMER=3647`，live consumer SHA-256
  `99225051f9120b6910dec91a9de1cdab2cc59f232db119a268bf3b50bbb52192`；Task5专用emit后的checked
  `rdbc-production-consumers.tsv` SHA-256为
  `514dd501caa6ffec8a0cb262d436deca6c029ef4b3e0f9eba0d52b220571074d`。
- semantic self/logical-live：`18/18 PASS + JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0`；semantic/live hashes
  保持`5d99b573...cdbd9 / f2e1d929...d9ed84`。Task3 purity为`103/103 PASS + STANDARD_FILES=29`。
- Task4 frozen coverage/runtime SHA-256保持
  `f7b3d4b429fe9e5c4bfe954082f0518d8e78635f6f7cc21bde1ffcd40f2307c9` /
  `106323e17904d377172986ce7464a54de60b4bb575e5f67e333e37f16188a3ec`；Task5 adjusted handoff的
  RBDC、SQLx default和HTTP_SQL all-features compiler coverage-v2各`15/15 PASS`，没有一般化hash bypass。
- focused：C3 `21/21`、registry `10/10`、metadata `3/3`，以及既有standard logical、raw owner、
  recycle、admission、Physical Filter/Stat/ResultSet/Callable回归均在最终三crate全量运行中通过。
- full：`druid`、`druid-rdbc`、`druid-wrapper` all-target/all-feature tests exit `0`；workspace
  all-target/all-feature check exit `0`；三crate scoped Clippy `-D warnings`、rustdoc `-D warnings`、
  doctest `0/1/0`、rustfmt、Black、Python compile与diff-check全部PASS。

真实外部RBDC/HTTP/libSQL/JDBC Agent服务仍为`NOT_RUN`；deterministic local与compiler coverage不冒充
external deployment evidence。当前状态是 **FIX_ROUND_4_IMPLEMENTED_PENDING_REVIEW**；只有独立re-review
可把R3-1/R3-2及C3/N1/I6判为ADDRESSED，Task 6继续阻塞。

## 17. Independent Round 4 re-review — REVIEW CLEAR

Independent scoped re-review of `d95ef45..b115287` returned **REVIEW CLEAR**：

- multi-result cleanup `ADDRESSED`：Statement/Prepared/Callable reject-second probes全部raw `3/3`
  exactly-once close，primary proxy error保持，cleanup failure留在旧generation retry ownership；
- cancellation safety `ADDRESSED`：aborted `close_all`、rejected registration与successor close不能丢失
  registry ownership或留下leaderless `Closing`；Weak coordinator + shutdown signal在holder drop后不保留
  task/resource；无mutex跨await/callback；
- I6 `ADDRESSED`：28 exact runtime identities、permit-before-raw mutation和staged-ownership checks均有效；
- C1/C2/I1-I5无回归；新finding为`0 Critical / 0 Important / 0 Minor`。

Fresh independent gates：verifier `26/26`、C3 `21/21`、registry `10/10`、metadata `3/3`，三crate
all-target/all-feature、workspace check、Clippy/docs/fmt/diff，classifier `180/75/108`、semantic
`52/17/0`、denominator `73/57/3647`及RBDC/SQLx/HTTP coverage `15/15`均PASS。

Final status：**TASK5_COMPLETE_REVIEW_CLEAR**。Task 6可以开始。
