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
