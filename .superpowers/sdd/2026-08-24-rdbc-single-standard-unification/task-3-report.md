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
