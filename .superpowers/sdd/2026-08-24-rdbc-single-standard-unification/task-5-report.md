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
