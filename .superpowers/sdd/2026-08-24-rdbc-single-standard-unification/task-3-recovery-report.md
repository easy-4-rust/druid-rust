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

## 8. Independent review correction / recovery fix round 1/5

本段 append-only correction明确 supersede上一节“已满足 brief清单”的实现者判断。

- 独立审查事实源：`task-3-recovery-review.md`。
- Reviewed recovery commit：`53354859615c5f582c180048913476e96a8bd4e6`。
- Verdict：`FIX_REQUIRED`。
- Wrapper permanent contract：`ADDRESSED`。
- Wrapper hook documentation / unreachable arms Minor：`ADDRESSED`。
- Reachable `ModuleScopeIndex`：`PARTIAL`，仍有 `1 Important open`；无新 Critical。

独立 probe证明合法 function/impl generic `T::Item` 均被报：

```text
unknown path root in connection/contract.rs: T
```

根因是当前索引只有 module-level items/bindings/prelude，没有为 fn/impl/trait/type及相关声明
记录 lexical type/const generic bindings与 half-open source ranges。本 fix round 只处理该
Important：range内允许已声明 `T::Assoc` / const-generic相关路径，range外保持 fail closed；
generic bounds中的 external/unknown/transitional paths继续走 canonical policy。Task 4保持 blocked。

### Fix round 1 strict TDD RED

仅新增永久 purity fixtures，未修改 verifier production：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 39 tests
FAILED (errors=5)
```

`34 PASS / 5 RED`。五个合法正向分别是 function generic、impl generic、trait generic、type
alias generic、lifetime + type generic；全部稳定失败于：

```text
VerificationError: unknown path root in connection/contract.rs: T
```

独立 negative controls在同一旧实现上已经按预期通过：undeclared `T::Item`、
`<T as sqlx::ExternalTrait>::Item`、unknown generic bound、`crate::resource` transitional bound、
function generic range结束后泄漏。Const generic作为 type-rooted associated path argument的正向也通过。
因此 RED 原因是缺失 lexical type binding，不是 fixture语法、external policy或 module graph错误。

补充 struct/const generic probes 后新增 `2` 个 focused RED：struct field中的合法 `T::Item`
同样误杀；而函数 `const WIDTH` 被旧 `scan_scope` 错记为 module-level const item，导致非法
`WIDTH::Item` 被漏放。Fix round 1有效 RED总数因此为 `7`：六个 lexical type-generic正向误杀，
一个 const-generic range泄漏。最小修复必须既建立range binding，也阻止generic declaration
tokens污染module-level local item表。

自审的 nested-item probe再发现 `1` 个 focused RED：function generic `T` 被错误允许进入内部
`fn nested()`，但 Rust nested item不能捕获 enclosing function generic。有效 RED总数更新为 `8`。
Impl/trait generics必须继续对associated items可见，因此修复需要 declaration hierarchy barrier，
不能简单禁止所有嵌套range。

Bound-policy自审再新增 `2` 个 focused RED：inline `T: MissingBound` 与 where-clause
`where T: MissingBound` 均被大小写漏放。有效 RED总数更新为 `10`。因此“bounds继续走
canonical policy”不仅覆盖qualified paths，还必须解析bare bound roots；合法local/prelude/type
generic bounds继续允许。

### Fix round 1 GREEN implementation

Verifier现在新增两层range模型：

1. `LexicalGenericScope`记录 fn/impl/trait/type/struct/enum/union declaration kind、source、
   half-open token range、type/const/lifetime bindings。`T::Assoc` 仅在最内层有效 type binding
   range内放行；const binding作为path root明确拒绝，lifetime保留但永不成为path root。
2. `DeclarationScope`作为nested-item barrier：enclosing function generic不能进入内部 item；
   impl/trait generic仍可供associated methods/types使用。Function/impl generic在 declaration结束后
   不泄漏。

Module first pass跳过完整item headers/bodies与 `macro_rules` definitions，避免把 `const WIDTH`
generic错误登记为module item。Lexical index排除macro-generated token ranges，不把macro matcher中的
`impl<$generic>`当真实源码声明。

`GenericBoundSite`独立提取inline generic与top-level where predicates的bare/qualified bound roots：

- local items、prelude traits、其他合法type generic可以解析；
- unknown bare/qualified、external backend与 `compat/resource/spi` transitional roots继续使用同一
  canonical resolver并 fail closed；
- `<T as sqlx::ExternalTrait>::Item` 仍由普通qualified-path scan拒绝。

Focused GREEN：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 47 tests ... OK

python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
STANDARD_FILES=29
PURITY_PHASE=transitional
```

### Fix round 1 fresh gates

| Gate | Result |
| :--- | :--- |
| Python `py_compile` | PASS |
| purity self/live | `47/47`；`STANDARD_FILES=29` |
| druid-rdbc all-target/all-feature | `20/20`，其中 Wrapper `3/3` |
| JDBC semantic self/live | `13/13`；`52/17/UNKNOWN=0` |
| Task 2 identity/live | `37/37`；`73/55/3559` |
| consumer SHA | `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59` |
| DruidError transition | `4/4` |
| external RDBC consumer | `2/2` |
| workspace all-target/all-feature check | PASS |
| scoped Clippy `-D warnings` | PASS |
| rustdoc / doctest | PASS / `1/1` |
| rustfmt / diff check | PASS |

自审未发现新的 scoped correctness finding。没有修改 Wrapper production、Task4、Physical、Cargo、
admin或verification artifacts。Known admin broad-test failure未运行、未伪报GREEN。本轮状态为
`FIX_ROUND_1_IMPLEMENTED_PENDING_REREVIEW`；原 reviewer复核前 Task 4继续 blocked。

## 9. Recovery fix round 1 review correction / round 2/5

本段 append-only correction supersede“自审未发现新finding”。原 reviewer对 `7f0154f` 的结论为
`FIX_REQUIRED`：round 1 `PARTIAL`，仍有 `1 Important open`，无新 Critical/Minor。

开放子路径：

- nested function自己的shadow generic合法，但outer call `inner::<T>()` 缺少block-local item binding；
- 普通path scan没有跳过完整 `macro_rules!` definition range；
- `LIFETIME + ::` 未被candidate parser显式拒绝。

Round 2只修这三项；nested item非法捕获outer generic、unknown macro invocation、external macro
path与合法lifetime/HRTB controls必须保持。Task 4继续 blocked。

### Fix round 2 strict TDD RED

只新增永久fixtures，production verifier未改：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 54 tests
FAILED (failures=1, errors=3)
```

`50 PASS / 4 RED`：

1. nested generic shadow + outer call报 `unknown path root ... inner`；
2. nested local function/struct/type control首先报 `unknown path root ... helper`；
3. 未调用、多arm、nested-brace macro definition中的 `T::Item` 报 `unknown path root ... T`；
4. 非法 `LIFETIME + ::` (`'a::Item`) 未抛错，被错误 ACCEPT。

同一旧实现上，nested binding range泄漏、nested item非法捕获outer T、actual external macro、
unknown module-scope macro、char literal/loop label与既有47项均按预期。因此 RED精确对应round 2
三个子路径，不是fixture语法或既有contract回归。

### Fix round 2 GREEN implementation

1. `LexicalItemScope` 从已有 `DeclarationScope` 推导nested fn/struct/trait/type/enum/union name，并
   绑定到最近 enclosing function/block half-open range。Block item在声明前后均可解析；离开block
   立即失效。Nested function自己的shadow generic有效，outer function generic仍被declaration
   barrier阻断，不能被nested item捕获。
2. `verify_file_paths` 把完整 `macro_rules! name <delimiter> ...` definition indexes加入covered
   ranges；include/extern/use/ordinary path第二遍均不解释unexpanded token body。Module-scope macro
   invocation gate未放松；definition外实际 `sqlx::query!`继续拒绝。
3. Candidate parser在任何普通path resolution之前显式识别 `Token(kind=LIFETIME) + ::` 并拒绝；
   CHAR与single-colon label不匹配该形状，合法lifetime/HRTB不回归。

Focused GREEN：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 54 tests ... OK

python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
STANDARD_FILES=29
PURITY_PHASE=transitional
```

### Fix round 2 fresh gates

| Gate | Result |
| :--- | :--- |
| Python `py_compile` | PASS |
| purity self/live | `54/54`；`STANDARD_FILES=29` |
| druid-rdbc all-target/all-feature | `20/20`，Wrapper `3/3` |
| JDBC semantic self/live | `13/13`；`52/17/UNKNOWN=0` |
| Task 2 identity/live | `37/37`；`73/55/3559` |
| consumer SHA | `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59` |
| DruidError / external consumer | `4/4`；`2/2` |
| workspace all-target/all-feature check | PASS |
| scoped Clippy `-D warnings` | PASS |
| rustdoc / doctest | PASS / `1/1` |
| rustfmt / diff | PASS |

自审未发现新的 scoped finding。Wrapper production、Task4、Physical、Cargo、admin与verification
artifacts均无diff；known admin broad failure未跑/未伪报。本轮状态
`FIX_ROUND_2_IMPLEMENTED_PENDING_REREVIEW`，Task 4继续 blocked。

## 10. Recovery fix round 2 review correction / round 3/5

本段 supersede“自审未发现新finding”。Reviewer对 `20581e7` 仍判 `FIX_REQUIRED`：lifetime
`ADDRESSED`，nested binding与macro definition `PARTIAL`；`1 load-bearing Important + 1 Minor`
open，无新 Critical。

- Important：fn/block bare/local macro invocation未受policy裁决，可展开transitional path且真实
  rustc可编译；必须索引所有definition-range外invocations，除显式allowlist外fail closed。
- Minor：nested item binding缺少kind/namespace，function name可冒充multi-segment type/module root。

Round 3只修macro invocation policy与namespace Minor；Task 4继续 blocked。

### Fix round 3 strict TDD RED

新增永久 verifier + minimal rustc fixtures，production verifier未改：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 60 tests
FAILED (failures=4, errors=1)
```

`55 PASS / 5 RED`：

1. module-defined `leak!()` 在fn内调用、body展开 `crate::resource::Blob`，rustc成功而verifier漏放；
2. function-local definition + nested-block `leak!()` 同样rustc成功且漏放；
3. nested block未知bare `mystery!()`漏放；
4. nested `fn sqlx(){}` 后 `sqlx::External` 被verifier漏放，rustc精确E0433；
5. nested local `mod local` + `local::External` rustc成功，但verifier误杀 `unknown ... local`。

Complex unused macro definition、qualified std/core与bare format controls、actual external macro、
module-scope unknown macro、nested shadow/range、lifetime/attribute/include及既有54项均按预期。
因此RED同时证明load-bearing invocation bypass与namespace Minor，不依赖无效Rust fixture。

首次live GREEN尝试另发现 `if !condition` 被token-only invocation index误判为 `if!`。新增最小
rustc正向后稳定RED；round 3有效RED总数更新为 `6`。修复必须排除non-raw Rust keyword + unary
`!`，但不得放松任何普通bare identifier macro。

### Fix round 3 GREEN implementation

1. `MacroInvocationSite`完整索引 `macro_rules` definition ranges外的named invocations，包括
   module/function/local definition、nested block与recursive token nesting。Finite allowlist只有：
   bare `format` / `impl_standard_wrapper`、exact `crate::impl_standard_wrapper`、qualified
   `std/core/alloc` roots；不做local-definition proof或通配，其他 invocation fail closed。
2. Definition ranges继续从ordinary include/extern/use/path与invocation scan排除；definition外
   actual external/unknown macros仍拒绝。Non-raw Rust keywords后 unary `!`不生成invocation，避免
   `if !condition`误判；raw identifier不使用关键字豁免。
3. `LexicalItemScope`新增 declaration kind与namespace。Nested fn属于value namespace，只能以
   single-root turbofish形状通过，`fn sqlx`不能授权 `sqlx::External`；struct/trait/type/enum/union
   属于type namespace，nested mod属于module namespace，均保持合法multi-segment paths。

真实rustc controls证明：两类macro leak本可编译、function namespace误用为E0433、nested local
module与qualified std/core macros可编译。

Focused GREEN：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 61 tests ... OK

python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
STANDARD_FILES=29
PURITY_PHASE=transitional
```

### Fix round 3 fresh gates

| Gate | Result |
| :--- | :--- |
| Python `py_compile` | PASS |
| purity self/live | `61/61`；`STANDARD_FILES=29` |
| druid-rdbc all-target/all-feature | `20/20`，Wrapper `3/3` |
| JDBC semantic self/live | `13/13`；`52/17/UNKNOWN=0` |
| Task 2 identity/live | `37/37`；`73/55/3559` |
| consumer SHA | `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59` |
| DruidError / external consumer | `4/4`；`2/2` |
| workspace all-target/all-feature check | PASS |
| scoped Clippy `-D warnings` | PASS |
| rustdoc / doctest | PASS / `1/1` |
| rustfmt / diff | PASS |

自审未发现新的scoped finding。Wrapper production、Task4、Physical、Cargo、admin与verification
artifacts无diff；known admin broad failure未跑/未伪报。状态
`FIX_ROUND_3_IMPLEMENTED_PENDING_REREVIEW`，Task 4继续 blocked。

## 11. Recovery fix round 3 review correction / round 4/5

本段append-only correction supersede上一节“自审未发现新的scoped finding”。独立review对
`00711d7`的结论仍为`FIX_REQUIRED`：namespace Minor已`ADDRESSED`，macro invocation Important
仍为`PARTIAL`。根因是round 3按canonicalized name allowlist放行，没有解析macro binding来源；
local/raw approved-name shadow与`std/core/alloc` alias均可编译地绕过purity。

Round 4基线为`0e7e60d55c6511e10c79219336d7cdd474594976`，只拥有purity verifier、self-test与
append-only recovery artifacts。没有修改Wrapper production、Task4、Physical、Cargo、admin或
verification manifests。

### Round 4 strict TDD RED

先只新增永久self-test，production verifier未改。每个攻击在purity断言前都由真实
`rustc 1.97.1 --edition=2021 --crate-type=lib`证明可编译；初始核心命令返回
`VerificationError not raised`，不是fixture语法错误。

有效RED共`23`个独立行为：

1. 四个reviewer反例：local `impl_standard_wrapper`、local `format`、raw approved name、private
   helper macro re-export后grouped `std` alias；
2. `core`/`alloc` alias variants、bare imported `format`/`impl_standard_wrapper`、same-name多跳
   `pub use` chain与`macro_use`注入；
3. function/block lexical alias、parent textual macro进入nested inline/external child，以及
   definition/const、type、impl、closure、nested block、cfg-disabled六个invocation位置；
4. 自审追加四个load-bearing RED：`cfg_attr(..., macro_export)`冒充direct export、正确module
   identity但错误source、crate-root macro edge未受canonical scan、function-local non-module
   `#[macro_export]`冒充canonical definition。

新增正向controls在implementation前已通过：真实`wrapper/wrapper.rs` direct export的
`crate::impl_standard_wrapper!`（即使local textual同名）、unshadowed `format/std/core/alloc`、
textual/use shadow range结束与definition前恢复、relative alias存在时`::std/core/alloc`绝对路径。

### Round 4 GREEN implementation

1. `MacroInvocationSite`保留完整path、absolute/raw flag与half-open invocation range；approved path
   不再依赖ordinary name resolver或字符串allowlist。
2. `MacroDefinition`记录definition source/range、module identity、module/lexical scope、direct
   `#[macro_export]`与raw状态；`ModuleScope`记录external/inline child的parent declaration source/index，
   因此textual macro严格按definition之后、block结束、later child propagation生效。
3. `LexicalUseBinding`记录function/const/closure/nested block内use binding的整个block range；
   `LocalItem`保留value/type/macro namespace，module与lexical root shadow不会退化成名字猜测。
4. `crate::impl_standard_wrapper!`只接受scope=`wrapper::wrapper`、source=`wrapper/wrapper.rs`、
   module-scoped、non-raw、direct `#[macro_export]`且crate root无same-name use/re-export的definition。
   bare同名一律fail closed并要求qualified form。
5. bare `format!`只在无active textual definition、module/block import/re-export及crate export shadow
   时使用标准prelude。relative `std/core/alloc`在放行前查module/use/local type-module root shadow；
   leading `::`按Rust 2021 extern-prelude绝对路径处理。raw invocation、`macro_use`、extern alias、
   conditional/non-local/wrong-source export全部拒绝。
6. crate-root与canonical source均进入macro provenance gate；unknown/bare/local/external invocation
   继续fail closed。Macro definition body不展开、不解释；unused复杂definition保持正向。

最终focused GREEN：

```text
python3 scripts/test_verify_rdbc_single_standard_purity.py
Ran 83 tests ... OK

python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
STANDARD_FILES=29
PURITY_PHASE=transitional
```

### Round 4 fresh gates

| Gate | Result |
| :--- | :--- |
| Python `py_compile` | PASS |
| purity self/live | `83/83`；`STANDARD_FILES=29` |
| druid-rdbc all-target/all-feature | `20/20`，Wrapper `3/3` |
| JDBC semantic self/live | `13/13`；`52/17/UNKNOWN=0` |
| Task 2 identity/live | `37/37`；`73/55/3559` |
| consumer SHA | `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59` |
| DruidError / external consumer | `4/4`；`2/2` |
| workspace all-target/all-feature check | PASS |
| scoped Clippy `-D warnings` | PASS |
| rustdoc / doctest | PASS / `1/1` |
| rustfmt / diff | PASS |

### Round 4 self-review / residual boundary

- Mutation coverage：退化为name-only、忽略raw、忽略module/block alias、放宽conditional/wrong-source/
  non-module export或跳过crate root，至少一个永久fixture会失败。
- 机制不模拟macro expansion；这不是“证明任意macro body安全”。只有命名的canonical export与
  unshadowed标准宏root可调用，其他definition外invocationfail closed；macro body继续由源码审查、
  pollution token gate及Rust compile contracts约束。
- Wrapper production、Task4、Physical、Cargo、admin和verification manifests均无diff。Known admin
  broad supervisor failure未运行、未伪报GREEN。
- 状态为`FIX_ROUND_4_IMPLEMENTED_PENDING_REREVIEW`。原reviewer返回`REVIEW CLEAR`前，Task 4继续
  blocked；本实现者不宣称Important已独立关闭。
