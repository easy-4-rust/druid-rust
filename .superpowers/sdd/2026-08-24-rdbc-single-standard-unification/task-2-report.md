# P04 Task 2 Report: JDBC semantic 与 integration 分母冻结

- 日期：2026-08-25（Asia/Shanghai）
- Rust baseline / execution HEAD：`d25ef68d9d05c39eb75cb953e46f97ccff6a5908`
- 规格：`docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`
- 证据等级：`V0_STATIC`
- 结论：Task 2 denominator GREEN；所有 target 仍为 `PLANNED_TARGET`，未声明 adapter 或 logical implementation 为 canonical `LIVE`。
- CodeGraph：仓库无 `.codegraph/`，未初始化。

## RED / GREEN

### RED（测试先行）

实现 generator/verifier 前执行：

```text
python3 scripts/test_verify_jdbc42_semantic_inventory.py
→ exit 1: ModuleNotFoundError: generate_jdbc42_semantic_inventory

python3 scripts/test_verify_rdbc_standardization_denominator.py
→ exit 1: ModuleNotFoundError: verify_rdbc_standardization_denominator

python3 scripts/verify_jdbc42_semantic_inventory.py
→ exit 2: verifier file did not exist
```

semantic verifier 实现、live artifact 生成前再次执行：

```text
python3 scripts/verify_jdbc42_semantic_inventory.py
→ exit 1: JDBC oracle not found: docs/verification/rdbc-first/jdbc42-oracle.json
```

这些失败分别由缺少实现和缺少 live inventory 引起，不是测试语法或夹具错误。

### GREEN

```text
python3 scripts/test_verify_jdbc42_semantic_inventory.py
→ 8 tests / OK

python3 scripts/test_verify_rdbc_standardization_denominator.py
→ 9 tests / OK

python3 scripts/verify_jdbc42_semantic_inventory.py
→ JAVA_SQL=52 / JAVAX_SQL=17 / UNKNOWN=0

python3 scripts/verify_rdbc_standardization_denominator.py
→ PHYSICAL_ROWS=73 / ADAPTER_ROWS=55 / CONSUMER_ROWS=3145
```

brief 原始无缓存生成命令也已执行成功：

```text
python3 scripts/generate_jdbc42_semantic_inventory.py \
  --archive-sha256 5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b \
  --semantic docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv \
  --oracle docs/verification/rdbc-first/jdbc42-oracle.json
→ JAVA_SQL=52 / JAVAX_SQL=17
```

## OpenJDK oracle

| Field | Frozen value / evidence |
| :--- | :--- |
| Tag | `jdk8u502-b07` |
| Commit | `7373384725418fa645fb4508ec05a869bc7fc3e7` |
| Archive URL | `https://github.com/openjdk/jdk8u/archive/7373384725418fa645fb4508ec05a869bc7fc3e7.tar.gz` |
| Archive SHA-256 | `5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b` |
| Archive size | `94,004,963` bytes |
| 69-source manifest SHA-256 | `94c4bcaeebd1681f0f2de0048ae4d95f77a96ef8d03ee4453c71358825b0d1e5` |
| Source paths | `jdk/src/share/classes/java/sql/*.java`; `jdk/src/share/classes/javax/sql/*.java` |
| Source-derived count | `java.sql=52`; `javax.sql=17` |

Generator 从 tar member 动态枚举根包 `.java` 文件，解析 package、public top-level declaration、继承/实现签名，并记录每个源文件 SHA-256。测试通过向合成 archive 增加源码证明 denominator 随 archive source entry 改变，而不是由手写 52/17 名单生成。语义 mapping 只在源码分母产生后覆盖每个发现对象；未知对象 fail closed。

## Frozen artifacts

| Artifact | Data rows | SHA-256 | State |
| :--- | ---: | :--- | :--- |
| `jdbc42-semantic-inventory.tsv` | 69 | `3a44c4e77259a667da11d228d1db78075a32af93bee691d2fc61ddcfbe64c2b6` | `52 + 17`, `UNKNOWN=0` |
| `jdbc42-oracle.json` | 69 source objects | `f0f0349d3894f583e0f75d30b199774dddc65b9611f0d7eb88716104f16a760f` | pinned archive + pinned source manifest + per-source hash |
| `rdbc-physical-standardization.tsv` | 73 | `523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1` | P03 48/48 preserved + 25 current resource public items |
| `rdbc-adapter-implementations.tsv` | 55 | `c7b581cbc90ca201e6f5f692ed7b7a4993a9f047a91cdd0c2ade93faccfb2b66` | current impl denominator; targets planned |
| `rdbc-production-consumers.tsv` | 3145 | `97ba4731aaaa9cb0d9ded1ac33fc269466cd73ae1c64bfd32d991ea9b88f3578` | all scanned production occurrences |

Physical classifications：`MERGE_STANDARD=37`、`RDBC_EXTENSION=10`、`DRUID_INTERNAL=10`、`CONCRETE_IMPL=16`。

Adapter ownership：`RDBC_CURRENT_IMPL=6`、`DRUID_LOGICAL_OR_PROXY=18`、`WRAPPER_ADAPTER=31`。这些是 current impl，不是 target LIVE；每行绑定一个 planned canonical trait 和 Task 3/4/5/7 owner。

Consumer occurrence kinds：`IMPORT=842`、`TYPE=1313`、`TRAIT_OBJECT=479`、`IMPL=153`、`CALL=358`。crate 分布：`druid-rdbc=1036`、`druid=1436`、`druid-wrapper=673`。`CALL` 只计 `Type::method(...)`，关联常量路径归入 `TYPE`。扫描对象是三个 crate 当前全部 529 个 production `.rs` 文件；历史 63/41/21 等固定观察值未作为门禁。

## Fail-closed self-tests

Self-tests 明确拒绝：

- semantic missing/duplicate/extra、52/17 计数漂移、unknown shape/state、空 canonical、伪 `LIVE` implementation path、archive hash 与 69-source manifest drift；
- P03 old-path 缩水、physical missing/duplicate/extra/unknown、unresolved old path、fake source path；
- adapter/consumer missing/duplicate/extra/fake path；
- 注释、普通字符串、raw string、byte raw string 误计；
- grouped use、raw identifier、crate/module/symbol alias 逃逸；
- standardized module wildcard import（无法精确解析时 fail closed）。

P03 48-row denominator 由 `git show d25ef68...:docs/verification/rdbc-first/rdbc-physical-standardization.tsv` 固定，不从当前可变 TSV 反推，因此不能通过同时删除源码行与 manifest 行制造自洽缩水。

## Additional verification

```text
python3 -m py_compile <five Task 2 scripts>
→ PASS（PYTHONPYCACHEPREFIX 指向 /tmp，仓库未生成 __pycache__）

emit-live → verify semantic → verify denominator → emit-live
→ BYTE_STABLE=PASS

P03 required old paths
→ 48/48 present

adapter/consumer source_path existence audit
→ ALL_REAL

git diff --check
→ PASS
```

## Changed files

- `scripts/generate_jdbc42_semantic_inventory.py`
- `scripts/verify_jdbc42_semantic_inventory.py`
- `scripts/test_verify_jdbc42_semantic_inventory.py`
- `scripts/verify_rdbc_standardization_denominator.py`
- `scripts/test_verify_rdbc_standardization_denominator.py`
- `docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv`
- `docs/verification/rdbc-first/jdbc42-oracle.json`
- `docs/verification/rdbc-first/rdbc-physical-standardization.tsv`
- `docs/verification/rdbc-first/rdbc-adapter-implementations.tsv`
- `docs/verification/rdbc-first/rdbc-production-consumers.tsv`
- `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task-2-report.md`

未修改 production Rust、Cargo manifest、Cargo.lock、分支、worktree 或远端。

## Self-review and concerns

- `rust_shape` 明确允许 69 Java objects 映射到 trait/DTO/enum/module/callback/runtime exclusion，不制造 69 个 Rust types。
- 69 行均为 `target_state=PLANNED_TARGET`；adapter/logical implementation path 均为 `-`，state 仅 `PLANNED` 或 `NOT_APPLICABLE`。
- `SQLPermission` 是唯一 `EXCLUDED_RUNTIME / JAVA_RUNTIME_ONLY`，替代 owner 为 `host::process_security_policy`；未伪造 Rust implementation。
- consumer scanner 使用仓库既有 Rust 2021 lexer；注释和 literal 不进入 identifier denominator。occurrence 的真实文件保存在 `source_path`，token ordinal 写入 `symbol` anchor，以区分同文件同 kind/symbol 的重复真实 occurrence。
- 该 inventory 证明当前 authored production source 的静态分母，不证明 macro expansion、编译器 name resolution、adapter behavior、logical lifecycle、真实数据库或 Java/Rust differential；这些仍由 Tasks 3–8 验证。
- `jdbc42-oracle.json` 被仓库全局 `*.json` ignore；提交时必须仅对这一明确目标文件使用 `git add -f`，不能修改 `.gitignore` 或漏提交 oracle。

## Fix round 1/5: scanner 与 canonical schema hardening

- 日期：2026-08-25
- 修复基线：`317e5997ab472ebca4aa444d73aeeaeba20a9284`
- 范围：Task 2 scripts/self-tests、重新生成的 inventory、本文追加；未修改 production Rust/Cargo，未进入 Task 3。
- Minor report tracking：按 review 指令 deferred。

### Root causes

1. 原 scanner 只展开显式 crate/use alias，不知道文件所属 module，因此 `self/super/crate` qualified path 无法绑定；为补相对 import 而加入的 same-leaf fallback 又会把 unrelated import 和 local shadow 误绑定为 RDBC。
2. receiver method syntax 不含 trait identifier。原 scanner 没有从字段/参数类型或 accessor return type 建立保守类型证明，因此 `physical_mut()?.commit()/rollback()` 等调用完全缺席。
3. JSON 使用普通 `json.loads`，重复 key 会被覆盖；top/package schema 与 canonical bytes 未严格检查。TSV renderer 未拒绝 delimiter/control，checked manifests 也缺少独立 canonical parser。
4. `src/*.rs` 内的 `#[cfg(test)]` body 若不排除，会把测试引用混入 production denominator；receiver 扫描会放大该误计。

### RED evidence

新增测试在修复前稳定复现：

```text
python3 scripts/test_verify_jdbc42_semantic_inventory.py
→ 3 failures:
  extra/duplicate JSON key accepted
  noncanonical JSON accepted
  CRLF/control-bearing semantic TSV accepted

python3 scripts/test_verify_rdbc_standardization_denominator.py
→ UFCS expected 19, found 0
→ typed physical_mut receiver expected, found 0
→ unrelated same-leaf import/local shadow produced false rows
→ delimiter-bearing source_path rendered successfully
→ parse_consumer_payload absent

cfg(test) focused RED
→ expected production receiver methods [commit], found [commit, rollback]
```

Review 提到 17 个 `super::PhysicalStatement::*`；fresh current-source token inventory 实际发现 19 个。修复不缩减当前源码，19/19 全部冻结。

### GREEN behavior

- module context 从 `crates/<crate>/src/...` 推导，并按 Rust 语义解析 repeated `super`、`self`、`crate`。
- source-proven re-export graph 从 exact physical declarations 和 `pub use` 闭包建立；relative path 只有命中该 graph 才绑定，完全移除 by-name leaf fallback。
- local declaration只有 `rdbc-physical-standardization.tsv` 记录的 exact `source_file` 可以绑定；unrelated import/local shadow 为 0 rows。
- trait method names从真实 trait body 动态派生。Receiver CALL 只有在显式 field/parameter type 或 accessor return type 唯一指向该 trait，且 method 属于该 trait时计入；不使用 method-name-only 推断。
- 当前真实专项门禁：19 个 `super::PhysicalStatement::*` UFCS、5 个 `dyn super::PhysicalPreparedStatement`、35 个 `physical_mut()` receiver calls（`commit=1`、`rollback=2`）。
- `#[cfg(test)]` item token ranges从 production denominator 排除；production fixture 保留 `commit`，test-only `rollback` 不计。
- JSON 每层 duplicate-key rejection、top/package/object exact key sets、canonical `indent=2 + sort_keys + final newline` bytes。
- Semantic/physical/adapter/consumer TSV 使用独立 parser；严格 header、field count、sort、duplicate、canonical bytes，并拒绝 tab/CR/LF、Unicode `Cc` controls（含 C0 与 DEL）。

### Regenerated denominator

| Artifact | Rows | SHA-256 | Change |
| :--- | ---: | :--- | :--- |
| `jdbc42-semantic-inventory.tsv` | 69 | `3a44c4e77259a667da11d228d1db78075a32af93bee691d2fc61ddcfbe64c2b6` | bytes unchanged; parser stricter |
| `jdbc42-oracle.json` | 69 source objects | `f0f0349d3894f583e0f75d30b199774dddc65b9611f0d7eb88716104f16a760f` | bytes unchanged; parser stricter |
| `rdbc-physical-standardization.tsv` | 73 | `523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1` | unchanged; P03 48/48 retained |
| `rdbc-adapter-implementations.tsv` | 55 | `c7b581cbc90ca201e6f5f692ed7b7a4993a9f047a91cdd0c2ade93faccfb2b66` | unchanged |
| `rdbc-production-consumers.tsv` | 3661 | `7ba19b461dd7417c25505096e6b677efd397ad5439d646db2f457792db6ec388` | source-proven relative/receiver rows; cfg(test) excluded |

Consumer kinds：`IMPORT=835`、`TYPE=1303`、`TRAIT_OBJECT=487`、`IMPL=153`、`CALL=883`。crate 分布：`druid-rdbc=1138`、`druid=1871`、`druid-wrapper=652`。其中 qualified anchors 37、typed receiver CALL 541。

Final gates：semantic self-tests `11/11`、standardization self-tests `16/16`、live semantic `52/17/UNKNOWN=0`、live denominator `73/55/3661`、Python compile `PASS`、emit/verify/emit `BYTE_STABLE=PASS`、P03 old paths `48/48`、`git diff --check` PASS。

### Remaining boundary

该修复仍是 `V0_STATIC / PLANNED_TARGET`。宏展开、编译器完整 name resolution、动态 dispatch runtime target、真实数据库行为与 Java/Rust differential 仍由后续 Tasks 验证；receiver inference 遇到无唯一显式类型证明时不猜测、不生成 row。

## Fix round 2/5: lexical receiver identity 与 shared cfg model

- 日期：2026-08-25
- 修复基线：`4628c17d273bfb3e5a0baa32972f9a0411a59b42`
- 范围：Task 2 standardization verifier/self-tests、consumer inventory、本文追加；未修改 production Rust/Cargo，未进入 Task 3。
- Deferred Minors：本轮不处理。

### Root causes and RED

原 `typed_receiver_proofs` 把整文件所有 field/parameter/local/accessor 压成一个 `name -> trait` 字典。不同 impl、method、match arm 与 nested block 中的同名 `physical` 相互覆盖。真实 `filter_chain_impl.rs` 因最后一个 Prepared proof 获胜，Connection 的 `create_blob/create_clob/create_n_clob/database_meta_data/warnings/clear_warnings` 和 Statement warning 分支缺失，Statement/Connection warning 又被误标为 Prepared。

新增 RED：

```text
filter_chain_impl real gate
→ Connection#create_blob expected 1, found 0

nested shadow fixture
→ expected Connection commit+rollback and Statement warnings;
  only Statement warnings survived whole-file overwrite

cfg predicate API
→ production_excluded_indexes absent

test-only public derivation
→ #[cfg(test)] TestOnly and #[cfg(all(test,...))] AllOnly entered public denominator
```

### Lexical receiver proof

- 每个 binding 记录 name、optional trait、declaration token、lexical start/end 与 declaration identity。
- field 只在 exact self-type impl scope 可见；parameter 只在 method body 可见；local/pattern binding 只在最内层 block/match arm 可见。
- unrelated/ambiguous binding 保留为 `trait_path=None` shadow，阻断外层同名 proof，而不是回退猜测。
- accessor 使用 `(impl self type, method name)` identity；callsite 仅接受 `self.<accessor>()` 且 return signature 唯一指向一个 current trait。
- enum variant payload 从定义派生，并把 match-arm pattern binding 分别绑定 Statement/PreparedStatement。
- 每个 receiver CALL 的 symbol anchor 写入 `binding=<field|parameter|local|pattern|accessor identity>`，使误归因可审计。

真实 `filter_chain_impl.rs` receiver gate：

| Trait | Required methods/count | Result |
| :--- | :--- | :--- |
| `PhysicalConnection` | blob/clob/nclob/metadata/warnings/clear = 6 | PASS |
| `PhysicalStatement` | warnings/clear = 2 | PASS |
| `PhysicalPreparedStatement` | warnings/clear = 2 | PASS |
| `PhysicalConnectionFactory` | existing create/create/close = 3 | PASS |

`DruidPooledConnection::physical_mut` 仍保留 35/35 exact accessor calls，含 `commit=1`、`rollback=2`。合成 `&Other` nested shadow 的内层 commit 为 0，外层 Connection commit/rollback 与独立 Statement warnings 均保留。

### Shared production cfg model

`production_excluded_indexes` 现由 public-item scan、re-export graph、trait-method derivation、consumer scan、adapter scan 与 receiver scan 共享：

- `cfg(test)`：必然依赖 test，排除；
- `cfg(all(test, ...))`：任一 all 条件必需 test，排除；
- `cfg(any(test, feature = ...))`：可由非-test 分支成立，不排除；
- nested all/any 使用“test=false 时是否仍可能成立”的保守递归规则；unknown/not predicate 不误排。

Test-only public type 和 trait method 不再进入 production derivation；`cfg(any(test, feature))` public type/method 保留。

### Round 2 regenerated denominator

| Artifact | Rows | SHA-256 | Change |
| :--- | ---: | :--- | :--- |
| semantic TSV | 69 | `3a44c4e77259a667da11d228d1db78075a32af93bee691d2fc61ddcfbe64c2b6` | unchanged |
| oracle JSON | 69 | `f0f0349d3894f583e0f75d30b199774dddc65b9611f0d7eb88716104f16a760f` | unchanged |
| physical TSV | 73 | `523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1` | unchanged; P03 48/48 |
| adapter TSV | 55 | `c7b581cbc90ca201e6f5f692ed7b7a4993a9f047a91cdd0c2ade93faccfb2b66` | unchanged |
| consumer TSV | 3507 | `93aee0e466d1d736ee033a099ef3ae8d19d9640f4d86d4e3f460587a033e7651` | lexical receiver correction |

Consumer kinds：`IMPORT=835`、`TYPE=1303`、`TRAIT_OBJECT=487`、`IMPL=153`、`CALL=729`。crate 分布：`druid-rdbc=1132`、`druid=1743`、`druid-wrapper=632`。typed receiver CALL 从 whole-file 误合并的 541 收敛为 lexical proof 387。

Round 2 仍只证明 `V0_STATIC / PLANNED_TARGET`。无法在 lexical callsite 唯一证明 declaration identity 或 trait 的 receiver 不生成 row，不用 method-name 或 whole-file leaf fallback 猜测。

Round 2 final gates：semantic self-tests `11/11`、standardization self-tests `20/20`、Python compile PASS、live semantic `52/17/UNKNOWN=0`、live denominator `73/55/3507`、emit/verify/emit `BYTE_STABLE=PASS`、P03 `48/48`、独立 `filter_chain_impl` receiver gate `10/10`、`git diff --check` PASS。

## Fix round 3/5: complete receiver scopes 与 cfg node satisfiability

- 日期：2026-08-25
- 修复基线：`b542fab8e13d7653afe9c9ed47fb8ead039d2fae`
- 范围：Task 2 standardization verifier/self-tests、consumer inventory、本文追加；未处理 deferred Minors，未修改 production Rust/Cargo，未进入 Task 3。

### RED evidence

```text
PhysicalConnectionLease<L> real gate
→ expected accessor receiver rows 52, found 0

self.field vs bare parameter
→ field Connection#commit missing; parameter Statement#warnings survived

destructuring/closure/control-flow fixture
→ expected Connection commit=2, found 0

two generic impl accessors
→ expected Holder commit + Other warnings, found 0

cfg double-not/cfg_attr/conjunction
→ DoubleNotOnly not excluded

cfg comma node
→ test-only field exclusion swallowed live sibling through later body boundary
```

后续防御性 RED 还证明：live parent 的 outer `#[derive]` 会让旧 walker 跳过 nested cfg field；untyped closure/control pattern 若不建立 `None` shadow，会继承 outer trait 产生伪 receiver row。

### Generic impl identity and receiver expression

- impl parser 先跳过 generic declaration；trait impl 从 top-level `for` 后取 self type，inherent impl 从 generics 后取 self type。
- self type 保留 qualified base + concrete/generic args；声明 generic 参数按位置标准化为 `_G0...`，因此 inherent `impl<L> PhysicalConnectionLease<L>` 与 trait `impl<L> ... for PhysicalConnectionLease<L>` 统一为 `PhysicalConnectionLease<_G0>`，不同 generic base 不合并。
- callsite receiver expression 区分 `self.field`、bare parameter/local/pattern 与 `self.accessor()`；field 只能命中当前 normalized impl identity，bare name 不回退到 field。
- parameter/local 支持 tuple/array destructuring；closure parameter 支持 typed proof 与 untyped `None` shadow。
- `if let`/`while let` body、match arm pattern 均拥有精确 lexical range；unknown payload 也产生 `None` shadow，避免借用 outer 同名 proof。
- accessor 以 `(normalized impl self type, accessor name)` 为 identity；无法唯一匹配 return trait 时不猜。

真实门禁：

| Gate | Result |
| :--- | :--- |
| `PhysicalConnectionLease<_G0>::physical_connection(_mut)` | `52/52` CALL rows |
| `DruidPooledConnection::physical_mut` | `35/35` retained |
| `filter_chain_impl` Connection/Statement/Prepared | `10/10` retained |
| generic Holder/Other same accessor | distinct Connection/Statement rows |
| nested `&Other` / untyped closure / unknown control pattern | inner false rows `0`; outer rows retained |

### cfg AST, satisfiability and node binding

- cfg predicate 解析为 `atom/test/all/any/not` AST；固定 `test=false`，通过 partial-evaluation DPLL 搜索其他 atoms，只有不可满足时才标 test-only。
- 正确处理 `not(test)` 保留、`not(not(test))` 排除、nested all/any 与多个 cfg attributes 的 conjunction；同一 feature atom 在多个 attributes 中共享赋值，可发现矛盾。
- `cfg_attr(cond, cfg(expr))` 转换为有效条件 `!cond || expr`；`cfg_attr(not(test), cfg(test))` 因 test=false 不可满足而排除。
- attribute group 绑定真实 next Rust node；按 top-level comma、semicolon 或 matched body 结束，field/variant/match-arm/statement 不吞 live sibling/following function。
- live attributed parent 不跳过 body，继续发现 nested cfg fields/variants；只有 parent 整体 test-only 时才跳过完整 node。
- public item、re-export、trait method、consumer、adapter 与 receiver derivation 全部复用同一 excluded token ranges。

### Round 3 regenerated denominator

| Artifact | Rows | SHA-256 | Change |
| :--- | ---: | :--- | :--- |
| semantic TSV | 69 | `3a44c4e77259a667da11d228d1db78075a32af93bee691d2fc61ddcfbe64c2b6` | unchanged |
| oracle JSON | 69 | `f0f0349d3894f583e0f75d30b199774dddc65b9611f0d7eb88716104f16a760f` | unchanged |
| physical TSV | 73 | `523db9479457a4c4a59eb9a3b783a5d76267c4835de7b9c063c6f7965b97fec1` | unchanged; P03 48/48 |
| adapter TSV | 55 | `c7b581cbc90ca201e6f5f692ed7b7a4993a9f047a91cdd0c2ade93faccfb2b66` | unchanged |
| consumer TSV | 3559 | `add1a61b90cfea2cff26d7ca632b651233d0efdd6e57df81f5404976cdf70729` | +52 real lease calls; corrected scopes |

Consumer kinds：`IMPORT=835`、`TYPE=1303`、`TRAIT_OBJECT=487`、`IMPL=153`、`CALL=781`。crate 分布：`druid-rdbc=1132`、`druid=1793`、`druid-wrapper=634`。receiver CALL `439`。

Round 3 仍只证明 `V0_STATIC / PLANNED_TARGET`；完整 rustc type inference、macro expansion、runtime dispatch、真实数据库与 Java/Rust differential 不由此静态 inventory 声明。

Round 3 final gates：semantic self-tests `11/11`、standardization self-tests `26/26`、Python compile PASS、live semantic `52/17/UNKNOWN=0`、live denominator `73/55/3559`、emit/verify/emit `BYTE_STABLE=PASS`、P03 `48/48`、真实 lease receiver `52/52`、`filter_chain_impl` receiver `10/10`、`git diff --check` PASS。
