# P04 Task 3 Breaker Recovery 报告

- 日期：2026-08-25
- Recovery base：`981336ac5c87b3e9d7e92e6cae691b21c4036c05`
- 分支：授权的当前 `main`
- 规格事实源：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`
- 恢复需求清单：`.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-3-recovery-brief.md`
- 范围：purity `ModuleScopeIndex`、永久 Wrapper external fixtures、Wrapper hook/macro hygiene
- 明确排除：Task 4、`Physical*`、Cargo manifests/lockfile、`druid-admin`、push

## 1. Preflight

- `HEAD` 与 recovery base 完全相同；`git status --short --branch` 只有
  `## main...origin/main [ahead 73]`，开始时无 tracked/untracked 修改。
- 仓库无 `.codegraph/`；按项目规则未初始化 CodeGraph。
- 已读取 recovery brief、完整规格、实施计划 Task 3、append-only ledger 与历史 Task 3 报告。
- 当前 verifier 仍以 `path.parent / "mod.rs"`、物理 `rglob` 与大小写 heuristics 判断路径；
  没有从 `lib.rs` 建立递归可达 module graph。
- 当前 Wrapper runtime 行为已有大部分实现，但仓库永久 fixture 未冻结完整 17 variants、六组
  inheritance、复杂 generic 及两类精确 compile-fail；两个 implementor hooks 未 hidden，macro
  尾部保留六个被早期 `$target:ty => $kind:ident` arm 吞掉的不可达旧 arms。

## 2. Strict TDD RED

先只修改 `scripts/test_verify_rdbc_single_standard_purity.py`，未修改 verifier 或 Rust
production code。新增 fixture 将每个 module/re-export/path 边界拆成独立 test。

命令：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
```

结果：exit `1`；`Ran 26 tests`；`14 PASS / 12 RED`，其中 `8 failures + 4 errors`。

真实 RED：

| 独立行为 | 旧实现的真实失败 |
| :--- | :--- |
| inline module | 合法 `mod inline { ... }` 被报 `module declaration escapes discovered canonical files` |
| nested inline module | 合法 nested inline 同样被误杀 |
| `foo.rs -> foo/child.rs` | verifier 错读 `foo/mod.rs`，抛出 `FileNotFoundError` |
| ordinary `path = 1` | 普通局部变量赋值被误报为 `#[path]` |
| ambiguous re-export | 两个同名 re-export 被漏放 |
| missing re-export | 未声明 physical file 被当成可达 binding，漏放 |
| hidden physical sibling | `super::hidden` 在父模块未声明仍被漏放 |
| parent transitional resource | `super::super::resource` 被漏放 |
| over-root `super` | 三次 `super` 越过 crate root 被漏放 |
| unknown uppercase root | `Mystery::Value` 因大小写 heuristic 被漏放 |
| unknown module-scope macro | `make_module!()` 的未知 module edge 被漏放 |
| conventional child diagnostic | 合法 child 产生 Python error，而不是语义 verifier verdict |

Direct/nested `cfg_attr(path)`、qualified `include!` 与现有正向 controls 在 RED 基线已经按预期
判定；它们保留为永久回归测试。下一节在 GREEN 后追加，历史 RED 不覆盖。

自审又新增并实跑两个 focused RED：

1. `#[sqlx::test]` backend procedural attribute 被 attribute-range skip 漏放；旧实现为 assertion
   failure。修复后 attribute 内除 `path = ...` 的专门检查外，qualified paths 继续走统一 resolver。
2. nested inline descendant 合法引用 parent private item 被错误要求 `pub`；旧实现报
   `item is not reachable`。修复后按 Rust privacy 规则允许 defining module 及其 descendants，
   sibling/ancestor access 仍要求可见 binding。

因此 recovery 的有效 purity RED 为首轮 `12` 个独立失败 + 自审 `2` 个 focused 失败，共
`14` 个独立行为。Wrapper fixture 开发过程中曾用错误的 `generics [<...>]` 写法多包一层尖括号，
并使用过窄的 dyn diagnostic substring；两者属于 fixture 自身错误，修正后才纳入合同证据，
不伪计为 production RED。

## 3. GREEN：reachable `ModuleScopeIndex`

`scripts/verify_rdbc_single_standard_purity.py` 现在从 `src/lib.rs` 建立两遍索引：

1. 第一遍递归读取 external modules，按 `foo.rs` / `foo/mod.rs` 唯一候选解析，并以
   `foo.rs -> foo/child.rs` 规则继续递归；同时为 inline/nested inline modules记录完整
   `crate::...` identity、source file 与 half-open token-index range。
2. 每个 module scope记录 declared children、visibility、local items、明确 `use/pub use`
   bindings/re-exports、module-scope macro edges与当前 module stack。
3. 第二遍解析 binding/re-export chain与 `crate/self/N×super/bare/absolute` paths；use、普通代码、
   macro与procedural attribute共用同一 target resolver。over-root、unknown root、缺失 child、
   duplicate/ambiguous binding、re-export cycle、transitional `compat/resource/spi` target均 fail closed。
4. `#[...]` 先按完整 delimiter range定位；任何 direct/nested `path = ...` 拒绝，普通
   `let path = 1` 不再误杀。Direct/qualified `include!` 全部拒绝。
5. Canonical roots下所有 physical `rglob("*.rs")` 仍进入 pollution denominator；只有从
   `lib.rs` declaration graph可达的 scopes可提供 relative-path legitimacy，未声明 sibling不因文件
   存在而成为 module。
6. Rust privacy按 defining module + descendants可见建模；跨 sibling/ancestor traversal要求公开
   child/binding/item。Docs、strings、comments、`std/core/alloc`、primitives、prelude与明确 canonical
   crate paths保留为正向 controls。

最终 purity GREEN：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 28 tests ... OK

python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
STANDARD_FILES=29
PURITY_PHASE=transitional
```

## 4. GREEN：永久 Wrapper external contract

新增 checked-in fixture source：

- `tests/fixtures/wrapper_runtime.rs`；
- `tests/fixtures/wrapper_wrong_variant.rs`；
- `tests/fixtures/wrapper_dyn_downcast.rs`。

`standard_wrapper_contract_test.rs` 通过临时 external package、offline Cargo与正常 integration
test target运行它们；fixture source本身永久受版本控制，动态 manifest不修改仓库 Cargo 文件。

Runtime fixture：

- 实例化并 exact variant + erased data-pointer identity 校验全部 `17` 个
  `StandardUnwrapped` variants；每个对象还单独校验 `dyn Wrapper` identity。
- 六组 inheritance 全部校验 variant与对象 identity：Callable -> Prepared -> Statement；
  Prepared -> Statement；DataSource -> CommonDataSource；ConnectionPoolDataSource ->
  CommonDataSource；XaDataSource -> CommonDataSource；XaConnection -> PooledConnection。
- 非 Connection 的 generic ResultSet 同时包含两个 type parameters、一个 const generic与 where
  predicate。
- unknown target固定为 `Ok(false)` / `SqlException` SQLState `0A000`；两个 `None` 输入固定为
  `InvalidArgument`；concrete `WrapperExt` downcast保留 exact identity。

Compile-fail fixtures：

- wrong canonical variant必须失败，并同时断言 `E0277`、`WrongVariant` 与 `Connection`；
- `dyn Wrapper` concrete downcast必须失败，并同时断言 `E0277`、完整 dyn type与 `Sized`。

Core API仍不暴露 `Any`、`as_any` 或 `Object` variant。`wrapper_view` / `canonical_unwrap` 已标为
`#[doc(hidden)]` implementor hooks；公开文档规定每个 concrete type只使用一次最具体 leaf、继承
自动展开，真正无关的 multi-role type必须手写唯一 `Wrapper` impl。删除六个被通用 dispatcher
遮蔽的旧 macro arms。

最终 Wrapper GREEN：`3 passed / 0 failed`。

## 5. Fresh verification gates

| Gate | Fresh result |
| :--- | :--- |
| Python `py_compile` | PASS |
| Purity self-tests/live | `28/28`；`STANDARD_FILES=29` |
| Wrapper permanent runtime/compile-fail | `3/3` |
| JDBC semantic self-tests/live | `13/13`；`JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0` |
| Task 2 denominator self-tests | `37/37` |
| Task 2 live denominator | `PHYSICAL=73 / ADAPTER=55 / CONSUMER=3559` |
| Consumer SHA-256 | `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59` |
| `druid-rdbc` all-target/all-feature | `20/20` |
| DruidError transition | `4/4` |
| external RDBC consumer | `2/2` |
| workspace all-target/all-feature check | PASS |
| scoped Clippy `-D warnings` | PASS after one test-harness path-format warning was fixed |
| `RUSTDOCFLAGS=-D warnings` rustdoc | PASS |
| doctest | `1/1` |
| rustfmt / unstaged diff check | PASS |

`cargo test --workspace --all-targets --all-features` 未重复执行；历史报告已经把
`druid-admin::admin_binary_test::grpc_bind_failure_cancels_http_sibling_and_exits_immediately`
记录为稳定可复现的 exact supervisor failure。Recovery brief明确要求不修改 admin且不得把 broad
workspace test伪报 GREEN；本恢复以 workspace all-target/all-feature `cargo check` 证明编译边界。

## 6. Files and scope proof

实现/测试文件：

- `scripts/verify_rdbc_single_standard_purity.py`
- `scripts/test_verify_rdbc_single_standard_purity.py`
- `crates/druid-rdbc/src/wrapper/wrapper.rs`
- `crates/druid-rdbc/tests/standard_wrapper_contract_test.rs`
- `crates/druid-rdbc/tests/fixtures/wrapper_{runtime,wrong_variant,dyn_downcast}.rs`

状态文件：

- 本 recovery report；
- existing Task 3 report 与 ledger只 append recovery 状态，不覆盖历史五轮记录。

明确无 diff：Cargo manifests/lockfile、`Physical*`、Task 4 wrapper integrations、`druid-admin`、
其他 verification artifacts。

## 7. Self-review and remaining risks

- Rust diff无 unsafe、共享状态、锁、额外依赖、manifest/API variant新增；object safety由现有
  dyn compile contracts与新增 negative fixture共同覆盖。
- Compile-fail test不接受任意失败；error code与目标 diagnostic subject均受断言。
- Python resolver不再以大小写或 physical sibling猜测 module identity；所有相对合法性来自
  reachable scope。Source ranges以 tokenizer的 half-open token indexes保存，而非 byte offsets；
  当前验证只依赖 containment，因此不损失判定语义。
- 已知非本恢复风险仍只有历史 `druid-admin` supervisor exact failure；未修改、未掩盖。
- Task 3 recovery implementation已满足 brief清单，但状态保持
  `RECOVERY_IMPLEMENTED_PENDING_INDEPENDENT_REVIEW`。原 reviewer 返回 `REVIEW CLEAR` 前，Task 4
  继续 blocked；不宣称 adapter/logical LIVE、Physical removal、JDBC compliance或 broad workspace
  test GREEN。
