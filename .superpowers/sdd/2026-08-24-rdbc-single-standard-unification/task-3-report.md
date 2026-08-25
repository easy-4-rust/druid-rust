# P04 Task 3 报告：独立、纯净的 single-standard traits

- 日期：2026-08-25
- 执行基线：`da3ba90fd5e40ab528b48cad011e1daba51670bd`
- 工具链：Rust/Cargo `1.97.1`；MSRV `1.95`；edition `2021`
- CodeGraph：仓库无 `.codegraph/`，按指令不初始化
- SDD 事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`
- 状态：`STANDARD_TRAITS_DECLARED_PHYSICAL_CUTOVER_PENDING`

## 1. Controller breaker prerequisite

### RED

在 production code 之前新增并实跑两类 absolute identity fixtures：

1. `crates/druid/src/child/mod.rs` 中 `self::Holder<T>` accessor 与
   `crate::Holder<T>` caller 被错误合并；旧 scanner 产生 1 条伪
   `PhysicalConnection#commit` receiver row，binding 退化为 `accessor:Holder<_G0>`。
2. 无 source/re-export proof 的 `external::Holder<T>` accessor 与当前 crate 的
   `crate::external::Holder<T>` caller 被错误合并；旧 scanner 产生 1 条伪
   `PhysicalStatement#warnings` receiver row，binding 退化为
   `accessor:external::Holder<_G0>`。

RED 命令退出 `1`，两个 fixtures 均为 assertion failure，失败原因就是错误 namespace
合并，不是 fixture 语法错误。

### Root cause 与 GREEN

`normalize_type_identity` 展开 `crate/self/super` 后又剥掉 current module/crate prefix，
同时未区分 extern-prelude bare path。修复后：

- repository-relative self type 保留完整 `<current-crate>::<module>::Type` identity；
- 显式 `use crate::...` proof 归一到同一 absolute identity；
- 无 proof 的 bare/external-prelude path 进入 `__unresolved__::...` namespace；
- local source declaration、local module、generic position 与 associated self identity 保持；
- 既有 lease/filter/closure/cfg scanner 行为不回归。

GREEN 证据：

| Gate | Result |
| :--- | :--- |
| semantic verifier self-tests | `13/13`（原 11 + DECLARED phase 2） |
| standardization verifier self-tests | `37/37`（原 34 + breaker/proof 3） |
| live semantic | `JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0` |
| live denominator | `PHYSICAL=73 / ADAPTER=55 / CONSUMER=3559` |
| five Task 2 scripts `py_compile` | PASS |
| emit/verify/emit | byte-stable |
| P03 immutable old paths | `48/48` |
| `PhysicalConnectionLease` receiver calls | `52/52` |
| `filter_chain_impl` Connection/Statement/Prepared matrix | `10/10` |
| `DruidPooledConnection::physical_mut` | `35/35`；commit `1`；rollback `2` |
| adapter/consumer source paths | `ALL_REAL` |

Consumer TSV 从 BASE 的
`add1a61b90cfea2cff26d7ca632b651233d0efdd6e57df81f5404976cdf70729`
再生为
`b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59`；row count
仍为 `3559`，变化来自 receiver owner 的 absolute identity，不是 denominator 增减。

## 2. Strict TDD：standard contracts

### RED

| Fixture | RED evidence |
| :--- | :--- |
| purity live scaffold | 29 个 canonical files 全部缺失，exit `1` |
| `standard_surface_test` | 8 个 standard modules unresolved，exit `101` |
| `standard_external_implementor_test` | external temp crate 的 connection/result_set/statement unresolved，exit `101` |
| `standard_handle_ownership_compile_test` | connection/statement/result_set unresolved，exit `101` |
| `standard_metadata_ownership_compile_test` | connection/metadata/result_set/statement unresolved，exit `101` |
| Physical target public-path extension | 10 组 companion descriptor/mapping names unresolved，exit `101` |
| JDBC Wrapper inheritance | 8 个 core dyn traits 未满足 `Wrapper` supertrait，exit `101` |

### GREEN

| Fixture | Result |
| :--- | :--- |
| purity verifier self-tests | `5/5` |
| live purity transitional | `STANDARD_FILES=29`，PASS |
| standard surface | `2/2` |
| external temp crate | `1/1`；`cargo check --offline`；仅依赖 `druid-rdbc` + `async-trait`，无 `druid` |
| owned Statement/ResultSet actor | `1/1` |
| owned live metadata + immutable snapshots | `1/1` |
| `druid-rdbc --all-targets` | `13/13` tests |
| doctest | `1/1` |

## 3. Public signatures 与 owner matrix

关键 object-safe signatures：

```rust
#[async_trait::async_trait]
pub trait Connection: Wrapper + Send + 'static {
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError>;

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError>;

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError>;

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn DatabaseMetaData>, RdbcError>;
}

#[async_trait::async_trait]
pub trait Statement: Wrapper + Send + 'static {
    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn ResultSet>, RdbcError>;
}

#[async_trait::async_trait]
pub trait ResultSet: Wrapper + Send + 'static {
    async fn next(&mut self) -> Result<bool, RdbcError>;
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError>;
}

#[async_trait::async_trait]
pub trait DatabaseMetaData: Wrapper + Send + 'static {
    async fn tables(
        &mut self,
        query: TablesQuery,
    ) -> Result<Box<dyn ResultSet>, RdbcError>;
}

pub trait ResultSetMetaData: Wrapper + Send + Sync + 'static { /* snapshot */ }
pub trait ParameterMetaData: Wrapper + Send + Sync + 'static { /* snapshot */ }
pub trait Driver: Send + Sync + 'static { /* URL negotiation */ }
pub trait ConnectionFactory: Send + Sync + 'static {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError>;
}
```

`PreparedStatement: Statement`、`CallableStatement: PreparedStatement`；所有 generic unwrap
helpers 位于 `WrapperExt`，不污染 runtime traits 的 dyn compatibility。

Ownership evidence 使用 cloneable `ActorHandle(Arc<AtomicUsize>)`：connection 被 drop 后
statement 仍能创建 result set；statement 被 drop 后 result set 仍能 `next(&mut self)`。Metadata
fixture 证明 connection drop 后 owned `DatabaseMetaData` 仍能查询，metadata drop 后 owned
result set 仍能推进；`Arc<dyn ResultSetMetaData>` 与 `Arc<dyn ParameterMetaData>` 在 producer
drop 后仍为 `Send + Sync + 'static`。

## 4. Canonical surface 与 inventory phase

创建 29 个 canonical files，覆盖：

- `connection`：`Connection` + `Savepoint` canonical re-export；
- `statement`：Statement/Prepared/Callable、typed options/parameter/generated-key/result DTO；
- `result_set`：owned cursor、1-based `ColumnIndex`、update descriptor；
- `metadata`：owned live metadata、immutable column/parameter snapshots；
- `driver`：Driver、ConnectionFactory、pure `DriverRegistry` 与 descriptor；
- `data_source`：Common/DataSource/Pooled/ConnectionPool/XA；
- `wrapper`：object-safe Wrapper、`StandardUnwrapped`、generic `WrapperExt`；
- `extensions`：immutable `ConnectionCapabilities` 与 standard `ExecResult` path。

Semantic inventory：

- `DECLARED=20`；
- `PLANNED_TARGET=49`（resource/RowSet/event/runtime 后续阶段，不制造 69 个 Rust types）；
- `ADAPTER_LIVE=0`；
- `LOGICAL_LIVE=0`；
- `UNKNOWN=0`；
- semantic SHA-256：
  `5d99b57378649dbe16378f9e3992cfed46058bdb12cf138847006dce430cdbd9`；
- oracle SHA-256：
  `46b39728593ceb91029dd40d0369e31b42de5de57a0ad0d7d52f165678630159`。

`DriverManager` 的 Rust shape 明确 disposition 到 `driver::DriverRegistry`，没有制造静态
Java manager class。Registry 只持 `Arc<dyn Driver>` / optional
`Arc<dyn ConnectionFactory>`，不引 `inventory`，不承载产品 validation、exception、reset 或
prepared policy。`cargo tree -p druid-rdbc` 不含 `druid` 与 `inventory`，Cargo manifests 与
lockfile均未修改。

## 5. Files

### Canonical/source/tests

- `crates/druid-rdbc/src/lib.rs`
- `crates/druid-rdbc/src/{connection,data_source,driver,extensions,metadata,result_set,statement,wrapper}/**/*.rs`
- `crates/druid-rdbc/tests/standard_{surface,external_implementor,handle_ownership_compile,metadata_ownership_compile}_test.rs`
- `crates/druid-rdbc/src/error/rdbc_error.rs`（Clippy identical-arm behavior-preserving merge）

### Verifiers/evidence

- `scripts/{verify,test_verify}_rdbc_single_standard_purity.py`
- `scripts/{verify,test_verify}_rdbc_standardization_denominator.py`
- `scripts/{verify,test_verify}_jdbc42_semantic_inventory.py`
- `docs/verification/rdbc-first/{jdbc42-semantic-inventory.tsv,jdbc42-oracle.json,rdbc-production-consumers.tsv}`

## 6. Verification gates

已运行：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
python3 scripts/test_verify_jdbc42_semantic_inventory.py
python3 scripts/verify_jdbc42_semantic_inventory.py --require-standard-entries
python3 scripts/test_verify_rdbc_standardization_denominator.py
python3 scripts/verify_rdbc_standardization_denominator.py
cargo test -p druid-rdbc --all-targets
cargo test -p druid-rdbc --doc
cargo clippy -p druid-rdbc --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p druid-rdbc --no-deps
cargo check --workspace --all-targets
cargo fmt --all -- --check
git diff --check
```

额外运行 `cargo test --workspace --all-targets`；它在未修改的
`crates/druid/tests/rdbc_external_consumer_test.rs::external_consumer_retains_druid_sql_public_contract`
失败。该历史 fixture 仍要求 P04 Task 1 已移除的
`druid_rdbc::compat::DruidError`，rustc 为 `E0425`。Task 3 不恢复已退出 waiver 的 compat
alias，也不把这个未授权的旧 consumer contract 改写混入 standard-traits commit；因此 full
workspace test 明确记录为 external pre-existing gate，不能列为 GREEN。受影响 workspace 的
全 targets 编译门禁 `cargo check --workspace --all-targets` 为 PASS。

## 7. Self-review concerns / remaining work

1. `spi::Physical*` 保持 transitional 且未 alias 到 method set 不同的新 traits；Task 6 cutover
   完成前不得删除，也不得宣称最终 completion gate 1 已满足。
2. 旧 `druid::spi` inventory descriptor/registry 继续承载产品 checker/sorter profile；本 Task
   新建的 `druid-rdbc::driver::DriverRegistry` 是独立 pure standard registry。未把产品 profile
   移入 standard，也未为移动而新增 Cargo dependency。后续 adapter cutover 应显式重命名/
   internalize 旧 product profile，避免长期同名认知负担。
3. 49 个 Java semantic rows仍是 honest `PLANNED_TARGET`；resource/RowSet/events 不在本 Task
   冒充 `DECLARED`。
4. 本 Task 不宣称 adapter/logical `LIVE`、完整 JDBC compliance、真实数据库行为、Java/Rust
   differential、Task 6 Physical removal 或 Task 7 stream hardening。
5. 标准 async 方法继续使用 workspace 既有 `async-trait` 的 Send future mapping；未修改
   Cargo manifest/MSRV。
6. Full workspace tests 尚受上述 stale `compat::DruidError` external-consumer fixture 阻断；
   这不是 standard trait 的 compile/object-safety/ownership failure，仍需 Task 1 owner 单独修正
   fixture 或提供兼容性裁决。

## Fix round 1/5：canonical error semantics 与 fail-closed surface

- 日期：2026-08-25
- 修复基线：`26fdcabbd40951094410ec9c5581421e76a83934`
- Reviewer findings：`2 Critical + 5 Important`
- 范围：Task 3 canonical traits/tests/purity verifier，以及 Task 1 已批准错误边界的 stale
  external/test fixtures；未进入 Task 4，未修改、迁移或删除 `Physical*` hierarchy。

### A. Strict TDD RED

| Finding | Fresh RED evidence |
| :--- | :--- |
| Wrapper canonical unwrap/error | desired `Result` signatures 与当前 `bool/Option` 产生 `E0053/E0308`；`StandardUnwrapped::Connection/Object` 缺失产生 `E0599` |
| Capability truthful defaults | `typed_standard_descriptors_have_neutral_defaults` 因 `transactions=true` assertion failure |
| Credential/close defaults | desired `Connection::is_closed -> Result` 与旧 bool 产生 `E0308`；mutation checks 依次证明 DataSource、ConnectionPoolDataSource、XaDataSource 各自会错误调用 credentialless path |
| 1-based/generated keys | signed `TryFrom<i64/i128>` 缺失、`ColumnIndexes(Vec<usize>)` type mismatch、`Statement::generated_keys` 缺失，共产生 `E0277/E0308/E0407/E0599` |
| Metadata value contracts | typed metadata index mismatch `E0053/E0308`；`ParameterNullability`、`RowIdLifetime::ValidOther/jdbc_name/TryFrom` 缺失 |
| Dynamic purity/import allowlist | extra `connection/hidden_backend.rs` 与 `use sqlx::Executor` 在固定 29-file verifier 下均未被拒绝，两个 adversarial assertions failure |
| Task 1 external consumer | focused test稳定复现 `druid_rdbc::compat::DruidError` 缺失 `E0425`；改用 root owner paths 后进一步证明 `druid::DruidError` / `druid_rdbc::RdbcError` 尚未 root-export `E0425/E0433` |

Credential mutation RED 使用同一行为 fixture 分三次逐个恢复错误实现，分别停在 DataSource、
ConnectionPoolDataSource、XaDataSource assertion；恢复正确实现后同一测试 GREEN，避免一个首个
assertion 隐藏后两个 overload。

### B. GREEN contracts

1. `Wrapper::is_wrapper_for(Option<TypeId>) -> Result<bool, RdbcError>`：`None` 为
   `InvalidArgument`，unknown target 为 `Ok(false)`。
2. `Wrapper::unwrap(Option<TypeId>) -> Result<StandardUnwrapped, RdbcError>`：`None` 为
   `InvalidArgument`，unsupported target 为 `SQLFeatureNotSupportedException` mapping；不再用
   `Option::None` 吞掉失败。
3. `StandardUnwrapped` 现在显式包含 Connection、Statement、PreparedStatement、
   CallableStatement、ResultSet、DatabaseMetaData、ResultSetMetaData、ParameterMetaData、
   Common/DataSource、Pooled/ConnectionPool/XA、Driver/ConnectionFactory、Wrapper canonical
   trait-object variants。具体 `Any` downcast 仅由 `WrapperExt::unwrap_ref` 暴露。
4. `ConnectionCapabilities::default()` 五项全部 false；doctest 同步。
5. 三个 credential overload 默认返回 typed feature-not-supported，不再丢弃 user/password；
   `Connection::close/is_closed` 默认返回 typed feature-not-supported，失败可观察。
6. `ColumnIndex` / `ParameterIndex` 提供 `TryFrom<i64>` 与 `TryFrom<i128>`，拒绝负数、0 和
   platform overflow；GeneratedKeys 使用 `Vec<ColumnIndex>`；ResultSetMetaData/
   ParameterMetaData APIs 使用 typed indexes；Statement 返回 owned
   `Box<dyn ResultSet>` generated keys。
7. `ParameterNullability` 完整表达 NoNulls/Nullable/Unknown；RowIdLifetime 包含五个 JDBC
   4.2 values（含 `ROWID_VALID_OTHER`），并提供 exact JDBC name 双向转换。
8. Purity verifier 从 `lib.rs` public canonical roots 动态发现 module tree，并对每个 root
   `rglob("*.rs")`；新增 file/module 自动受检。Import gate 使用 tokenized use-tree 与
   std/core/alloc/async-trait、canonical crate module allowlist；拒绝 backend root、spi/compat/
   resource implementation import 与 wildcard，不扫描 doc text。
9. External consumer 现在实际 `cargo run`：证明 `TypeId` 不同、RdbcError 到
   `DruidError::Rdbc` exactly-once wrap、再 round-trip 回 RdbcError；未恢复 waiver/alias。

### C. Task 1 stale fixture convergence

Full all-feature workspace gate继续运行后发现四处旧测试仍期待 `DruidError::DriverError` 或
`DruidError::UnsupportedOperation`，而 Task 1 已冻结实际边界为
`DruidError::Rdbc(RdbcError::...)`。调用链从 Physical RdbcError 通过
`impl From<RdbcError> for DruidError` exactly-once wrap，生产实现正确；仅修正：

- `recycle_semantics_test::initialization_failure_closes_connection_and_releases_capacity`；
- `result_set_filter_semantics_test` 的 close/warnings/clear-warnings 三个行为测试。

Focused all-feature tests全部 GREEN；未修改 Physical trait/impl。

### D. Verification evidence

```text
Task 3 purity self-tests                    -> 7/7
live dynamic purity                         -> STANDARD_FILES=29 / PASS
semantic verifier                           -> 13/13; 52/17/UNKNOWN=0
Task 2 identity/denominator self-tests       -> 37/37
live denominator                            -> PHYSICAL=73 / ADAPTER=55 / CONSUMER=3559
consumer SHA-256                            -> b1b420d61a8c...1c72c59 (unchanged)
druid-rdbc all targets                      -> 17/17
druid-rdbc doctest                          -> 1/1
external consumer focused/full              -> PASS
workspace all-target/all-feature check      -> PASS
druid-rdbc all-feature Clippy -D warnings    -> PASS
RUSTDOCFLAGS=-D warnings cargo doc           -> PASS
fmt / diff check                            -> PASS
```

`cargo test --workspace --all-targets --all-features` 已三次用于收敛：所有 Druid/RDBC tests
在 stale fixture 修正后通过；最终仍被一个与本 diff 无调用关系的 admin supervisor test 阻断。

### E. Non-RDBC residual risk：admin supervisor

保留完整失败证据且未修改 admin/timeout：

1. workspace all-feature parallel run：`admin_binary_test` 4/15 deadline failures；
2. `cargo test -p druid-admin --all-features --test admin_binary_test -- --test-threads=1
   --nocapture`：14/15，仍失败 `grpc_bind_failure_cancels_http_sibling_and_exits_immediately`；
3. exact single-thread run：0/1，2.01s 稳定失败。

数据流是测试占用 gRPC address，spawn admin，预期 gRPC bind error 在 2 秒内取消 HTTP sibling
并退出；实际 child 保持运行且 stderr 为空。`git diff -- crates/druid-admin Cargo.toml
Cargo.lock` 为空；因此 workspace parallel load 只解释部分症状，不是唯一根因。这是既有、
可复现的非 RDBC admin supervisor 风险，不属于本 Task 授权修复范围；workspace test 不能伪报
GREEN。

### F. Fix-round self-review

- Reviewer 的 2 Critical + 5 Important 均有 RED/GREEN，当前 open finding：`0`。
- `Physical*` production files/traits/impl 未修改，Task 2 absolute current-crate identity ruling
  与 consumer bytes保持。
- Cargo manifests/lockfile未修改；未引入依赖。
- 仍不宣称 adapter/logical LIVE、Task 4 cutover、JDBC compliance 或 Task 6 removal。
- 原报告已记录的 `rdbc_error.rs` scope Minor 保持 deferred；本 fix round 不扩散处理。

### Round 1 re-review correction

Round 1 状态更正为 **PARTIAL**。先前“2 Critical + 5 Important 全部关闭 / open finding 0”
由本段明确 supersede。Re-review 仍有 `1 Critical + 2 Important open`：external implementor
canonical Wrapper identity、Statement/ResultSet/PooledConnection lifecycle default-failure、
以及 purity fully-qualified/macro/extern/path/include fail-closed boundary。其余 capabilities、
credentials/Connection lifecycle、indexes/generated keys/metadata、DruidError/RdbcError external
fixture 四组保持 ADDRESSED。

## Fix round 2/5：external canonical identity 与完整 purity boundary

- 修复基线：`63ab3f1e36bd8c7344cfa192b9ce77344e9e3bfe`
- Re-review：`1 Critical + 2 Important`
- 结果：三项均以 fresh RED→GREEN 关闭；未进入 Task 4，未修改 Physical/Cargo。

### RED / GREEN

1. **External Wrapper identity Critical**：真实 temp crate 首次 RED 为 macro 缺失 `E0433`、
   Connection/Statement/ResultSet 未满足 Wrapper `E0277`、canonical methods 缺失 `E0599`。
   GREEN 后 external implementor 只需单行
   `impl_standard_wrapper!(Type => CanonicalTrait)`；类型判断表只存在于 druid-rdbc macro，
   下游不复制。Wrapper core移除 `Any/as_any`；StandardUnwrapped 移除 `Object`；concrete
   downcast 仅在 `WrapperExt` 的 `Self: Sized + Any` 方法内。External binary实际运行并证明
   Connection canonical is/unwrap、unknown false/unsupported error、null InvalidArgument、具体
   downcast仅经 WrapperExt。
2. **Lifecycle Important**：external fixture 对 Statement/ResultSet `close/is_closed` 与
   PooledConnection `close` 的 desired Result contract产生 `E0308` RED；GREEN defaults全部返回
   typed feature-not-supported，不再 Ok/false 矛盾。
3. **Purity Important**：adversarial fixture 对 fully-qualified type path、extern crate、macro
   path、self backend module、`#[path]`、`include!` 六项在旧 verifier 下 assertion RED；grouped/
   alias/absolute use已由旧 gate拒绝。GREEN verifier tokenizes而不扫描doc/string/comment，动态
   tree一致扫描所有 files；覆盖 use ranges、普通qualified/macro paths、extern、relative module、
   path/include edge。正向 std grouped、crate grouped、doc/string controls通过。

### Fresh gates

```text
purity self-tests                       9/9
live purity                             STANDARD_FILES=29
druid-rdbc all-target/all-feature tests 17/17
semantic                               52/17/UNKNOWN=0
live denominator                        73/55/3559
consumer SHA-256                        b1b420d61a8c...1c72c59
workspace all-target/all-feature check PASS
external DruidError/RdbcError tests     PASS
Clippy -D warnings / rustdoc / fmt      PASS
```

已知 non-RDBC admin supervisor exact failure保持原记录，不重复 broad workspace test；未写
GREEN。Round 2 reviewer findings open：`0`。

### Round 2 re-review correction

Round 2 状态更正为 **PARTIAL**；先前 0-open 表述由本段 supersede。Re-review 仍有
`1 Critical + 1 Important open`：Wrapper macro 17 variants/supertrait assignability/generic
hygiene，以及 purity absolute/crate/relative/cfg_attr path policy。

## Fix round 3/5

- 基线：`568224c22dfdf325b2108c9b0520b881e42d94b8`
- Re-review：`1 Critical + 1 Important`；结果：`0 open`。
- Wrapper fresh RED：external macro缺失 `E0433`、Wrapper bounds `E0277`、lifecycle signature
  `E0308`。GREEN：17 StandardUnwrapped variants均有单次 macro arm；Prepared 自动声明
  Prepared+Statement assignability，Callable 自动声明 Callable+Prepared+Statement；generic
  syntax `impl<T: Bound> Type<T> => Connection` 经 external runtime通过；wrong/unknown/null保持
  structured failure。Core不公开Any/as_any/Object，concrete downcast仅WrapperExt。
- Purity fresh RED覆盖 fully-qualified/absolute backend、`crate::resource`、extern crate、macro、
  self backend、`#[path]`、nested cfg_attr path、include。GREEN统一use/non-use allowlist并按真实
  canonical tree解析 relative paths；合法 `super::Statement`、
  `super::super::statement::Statement`、std/crate grouped、doc/string/comment controls通过。
- Gates：druid-rdbc 17/17；purity 9/9/live29；workspace all-feature check PASS；Task2
  73/55/3559、consumer `b1b420d6...72c59`；Clippy/docs/fmt/diff PASS。Admin known risk未重跑。
- 未修改 Physical、Task4、Cargo manifests/lock。Round3 reviewer findings open：`0`。

### Round 3 re-review correction

Round 3 状态更正为 **PARTIAL**；先前 0-open 表述由本段 supersede。Re-review 仍有
`1 Critical + 1 Important open`：Wrapper 17 variants/完整继承closure/generic/wrong-variant
永久fixture，以及 purity 真实 ModuleScopeIndex。

## Fix round 4/5

- 基线 `84791d7`；re-review `1 Critical + 1 Important` addressed，open `0`。
- Wrapper macro覆盖17 kinds与Wrapper identity；Callable/Prepared、DataSource/Common、
  ConnectionPool/XaDataSource/Common、XaConnection/Pooled继承closure；generic external runtime
  通过，并提供显式 generics/where syntax。
- Purity改用mod.rs真实声明索引，不按物理file/uppercase放行；absolute/crate/cfg_attr/include
  与合法super controls永久测试。
- druid-rdbc 17/17、purity9/9/live29、Clippy/fmt/diff PASS；Task2 bytes保持；admin未重跑。

### Round 4 re-review correction

Round 4 状态更正为 **PARTIAL**；先前 `open 0` 表述由本段 supersede。独立 re-review
确认 Wrapper 运行时行为已满足 17 exact variants、6 组 inheritance closure、非 Connection
multi-type/const/where generic 与 wrong-variant `E0277`，但这些关键合同没有全部固化到仓库永久
fixture。Purity 仍未建立从 crate root 递归可达的真实 `ModuleScopeIndex`：inline/nested/
conventional modules、显式 re-export、任意 `self/super` 与 attribute-scoped `path` 解析仍存在
可复现漏检或误杀。当前开放为 `2 Important + 1 Minor`。

## Fix round 5/5 breaker

- 基线：`b345ad07bee0e4acd3dd16ab675d81d35c3434ce`。
- 实现者没有提交 partial 修复；HEAD 与工作树保持 clean，未 push。
- 原因：剩余工作要求递归 Rust module graph、独立可失败 purity fixtures、17-variant external
  runtime、精确 wrong-variant/dyn-core compile-fail 与完整门禁，无法在本轮执行窗口内安全完成。
- 最终状态：**TASK3_FIX_REQUIRED_BREAKER**。

开放 blocker：

1. **Important — purity ModuleScopeIndex**：从 crate root 递归解析 external/inline/conventional
   modules、声明子模块、显式 re-export 与当前 module stack；缺失/歧义/越 root fail-closed；
   `path` 仅在 attribute token range 中解释；永久覆盖 hidden sibling、parent transitional root、
   over-root、ordinary assignment、nested `cfg_attr(path)` 与 qualified `include!`。
2. **Important — Wrapper permanent evidence**：把 17 exact variants + Wrapper、6 组 inheritance
   closure、unknown/None、非 Connection multi-type/const/where generic、concrete WrapperExt、dyn
   core 与 wrong-variant 精确 compile-fail 纳入常规仓库门禁。
3. **Minor — Wrapper hygiene**：删除失效旧 macro arms，并明确 implementor-only hook和“每个 concrete
   type只声明一个最具体 leaf”的公共文档。

Task 4 不得开始，直到用户批准 recovery ruling、两个 Important 均修复且独立 re-review 返回
`REVIEW CLEAR`。已知 `druid-admin` supervisor exact failure继续作为非 RDBC 外部风险记录，不能
把 workspace broad tests伪报为 GREEN。

## Breaker recovery implementation（append-only）

- Recovery base：`981336ac5c87b3e9d7e92e6cae691b21c4036c05`；用户批准当前 `main` atomic
  recovery；未使用 worktree、branch switch、reset、amend 或 push。
- Purity fresh RED：首轮 26 tests 中 `14 PASS / 12 RED`；自审另有 backend attribute漏放与
  descendant-private-parent误杀两个 focused RED。最终 reachable `ModuleScopeIndex` self-tests
  `28/28`，live `STANDARD_FILES=29`。
- Wrapper永久证据：`3/3`；17 variants + Wrapper exact identity、六组 inheritance、复杂
  ResultSet generics、unknown/None、concrete WrapperExt、wrong-variant E0277、dyn-core E0277。
- Wrapper hooks已 `#[doc(hidden)]`，macro usage/multi-role规则已写入公共文档，六个不可达旧 arms
  已删除；core仍无 `Any/as_any/Object`。
- Fresh gates：semantic `13/13 + 52/17/0`；Task2 `37/37 + 73/55/3559`；consumer SHA
  `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59`；druid-rdbc
  `20/20`；DruidError `4/4`；external consumer `2/2`；workspace all-feature check、Clippy、
  rustdoc、doctest、fmt/diff均 PASS。
- 完整实现/RED/GREEN/命令证据：`task-3-recovery-report.md`。
- 状态：`RECOVERY_IMPLEMENTED_PENDING_INDEPENDENT_REVIEW`；历史 admin supervisor风险不变；
  Task 4在原 reviewer 返回 `REVIEW CLEAR` 前继续 blocked。

### Recovery independent-review correction / fix round 1/5

- 审查事实源：`task-3-recovery-review.md`；reviewed commit `5335485`；verdict
  `FIX_REQUIRED`。
- Wrapper evidence与hook/dead-arm Minor均 `ADDRESSED`；reachable `ModuleScopeIndex` 为
  `PARTIAL`，`1 Important open`，无新 Critical。
- 独立复现：function/impl generic合法 `T::Item` 均被 purity verifier误杀为
  `unknown path root ... T`。根因是缺少 declaration-range lexical type/const generic bindings。
- 本轮基线 `5335485`；严格 RED→GREEN，仅修 lexical generic scope；Task 4、Physical、Wrapper
  production、Cargo、admin保持不动。Task 4继续 blocked。

### Recovery fix round 1/5 implementation

- 有效 fresh RED `10`：六组合法 type-generic `T::Item`误杀、const generic module-scope泄漏、
  function generic进入nested item、inline/where unknown bare bound漏放。
- GREEN：`LexicalGenericScope + DeclarationScope + GenericBoundSite`；type/const/lifetime分类、
  exact declaration ranges、nested function item barrier、inline/where bound canonical resolution。
- Purity `47/47 + live29`；Wrapper `3/3`保持；完整semantic/Task2/error/workspace check/
  Clippy/rustdoc/doctest/fmt gates均PASS，73/55/3559与consumer SHA不变。
- Scope：只修改 purity verifier/tests与append-only recovery artifacts；review事实源
  `task-3-recovery-review.md`纳入交付。状态 `FIX_ROUND_1_IMPLEMENTED_PENDING_REREVIEW`，Task 4
  继续 blocked。

### Recovery fix round 1 re-review correction / round 2/5

- Reviewer verdict仍为 `FIX_REQUIRED`；round 1 `PARTIAL`，`1 Important open`，无新
  Critical/Minor。
- 三个子路径：nested item binding/call、macro_rules definition普通path scan、lifetime associated
  root漏拒绝。
- Round 2基线 `7f0154f`，只修上述purity边界；Task 4、Physical、Wrapper production、Cargo、
  admin保持不动，Task 4继续 blocked。

### Recovery fix round 2/5 implementation

- Fresh RED `4`：nested shadow/call、nested local item matrix、macro definition body误扫、lifetime
  root漏拒绝。
- GREEN：function/block `LexicalItemScope`、完整 macro definition covered ranges、显式
  `LIFETIME + ::` rejection；range/capture与actual/unknown macro负向保持。
- Purity `54/54 + live29`；其余完整recovery gates全PASS，73/55/3559与consumer SHA不变。
- 状态 `FIX_ROUND_2_IMPLEMENTED_PENDING_REREVIEW`；Task 4继续 blocked。

### Recovery fix round 2 re-review correction / round 3/5

- Reviewer verdict `FIX_REQUIRED`：lifetime addressed；nested/macro partial；`1 load-bearing
  Important + 1 Minor` open，无新 Critical。
- Important是fn/block bare macro invocation可编译地展开transitional path；Minor是nested function
  binding被误当type/module namespace。
- Round 3基线 `20581e7`，执行显式macro invocation allowlist与LexicalItemScope kind/namespace；
  Wrapper/Task4/Physical/Cargo/admin不动，Task 4继续 blocked。

### Recovery fix round 3/5 implementation

- Fresh RED `6`：两类可编译macro leak、unknown nested invocation、function namespace误放、local
  module误杀、keyword-negation false positive。
- GREEN：definition外`MacroInvocationSite` + finite allowlist；LexicalItemScope kind/value-type-module
  namespace；non-raw keyword unary-not排除。
- Purity `61/61 + live29`；完整recovery gates全PASS，73/55/3559与consumer SHA不变。
- 状态 `FIX_ROUND_3_IMPLEMENTED_PENDING_REREVIEW`，Task 4继续 blocked。

### Recovery fix round 3 re-review correction / round 4/5 implementation

- Reviewer对`00711d7`仍判`FIX_REQUIRED`：namespace Minor addressed；macro allowlist只证明name、
  不证明binding provenance，仍有`1 load-bearing Important`。Round 4基线为`0e7e60d`。
- Fresh RED共`23`个真实rustc可编译行为，覆盖local/raw/import/re-export approved-name shadow、
  std/core/alloc grouped/alias/absolute roots、lexical/inline/external scope、六类source position、
  same-name pub-use chain、conditional/wrong-source/non-module export与crate-root edge。
- GREEN改为`MacroDefinition + MacroInvocationSite + LexicalUseBinding` provenance resolution；
  canonical wrapper macro必须是`wrapper/wrapper.rs`中`wrapper::wrapper` module-scope direct
  `#[macro_export]`，bare同名fail closed；unshadowed format/std/core/alloc保持。
- Purity `83/83 + live29`；完整semantic/Task2/error/Rust quality gates PASS，73/55/3559与consumer
  SHA不变；known admin broad failure未跑/未伪报。
- Scope只有purity verifier/tests与append-only artifacts；状态
  `FIX_ROUND_4_IMPLEMENTED_PENDING_REREVIEW`。Task 4继续blocked。
