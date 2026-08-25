# P04 Task 3 breaker recovery — independent review

Review base: `981336ac5c87b3e9d7e92e6cae691b21c4036c05`

Reviewed recovery commit: `53354859615c5f582c180048913476e96a8bd4e6`

Verdict: **FIX_REQUIRED**.

## Historical finding disposition

| Finding | Disposition |
| --- | --- |
| Important — permanent Wrapper contract evidence | ADDRESSED |
| Important — reachable ModuleScopeIndex | PARTIAL |
| Minor — Wrapper hook documentation and unreachable arms | ADDRESSED |

No new Critical finding exists. One Important sub-path remains inside the original
ModuleScopeIndex finding.

## Wrapper evidence

The normal `cargo test -p druid-rdbc --all-targets --all-features` gate runs the checked-in
`standard_wrapper_contract_test.rs` and passes 3/3. The fixtures prove 17 distinct concrete objects,
17 exact `StandardUnwrapped` variants, `Wrapper` identity, all six inheritance groups, complex
non-Connection ResultSet generics, unknown/None SQLState behavior, safe concrete WrapperExt,
wrong-variant E0277, and dyn Wrapper/Sized E0277. Hooks are doc-hidden and documented; obsolete arms
are gone. This finding is ADDRESSED.

## Remaining Important — lexical generic bindings

The new ModuleScopeIndex implements the recovery contract for reachable external/inline/nested/
conventional modules, source ranges, children, visibility, local items, imports/re-exports/macro
edges, missing/ambiguous/cyclic/unknown fail-closed behavior, crate/self/N-super resolution,
attribute-scoped path handling, include rejection, and the 29-file pollution denominator.

However, an independent associated/generic probe found legal Rust is rejected:

```rust
pub trait Assoc { type Item; }

pub fn f<T: Assoc>() {
    let _: Option<T::Item> = None;
}

pub struct S<T>(T);
impl<T: Assoc> S<T> {
    pub fn f(_: T::Item) {}
}
```

Both function-generic and impl-generic cases fail with:

```text
unknown path root in connection/contract.rs: T
```

Root cause: `resolve_path` sees module-level items/bindings/prelude only. It does not index lexical
type/const generic parameters declared by fn/impl/trait/type source ranges. `T` is a range-scoped
type binding, not an unknown module root.

Required fix:

1. Record type/const generic bindings for fn/impl/trait/type and relevant declarations with their
   lexical token ranges.
2. Permit `T::Assoc` only while the use is inside that binding's range.
3. Continue resolving bound paths under the canonical policy.
4. Add permanent positive tests for function and impl `T::Item`.
5. Add permanent negative tests for undeclared `T::Item`, `<T as sqlx::ExternalTrait>::Item`, and
   a generic bound that uses an unknown or transitional root.

This remains Important and blocks Task 4 because Task 4 may introduce legitimate generic standard
adapter paths.

## Independent gates that passed on 5335485

- purity self-tests 28/28; live `STANDARD_FILES=29`
- druid-rdbc all-target/all-feature 20/20; Wrapper permanent fixture 3/3
- semantic 13/13 and 52/17/UNKNOWN=0
- Task 2 identity 37/37 and live 73/55/3559
- consumer SHA `b1b420d61a8c5824d45eb3c59feecedb10795db803b4b3d20ece0509a1c72c59`
- DruidError transition 4/4; external RDBC consumer 2/2
- workspace all-target/all-feature check, scoped Clippy -D warnings, rustdoc/doctest, rustfmt,
  diff checks

No Task4, Physical, Cargo, verification artifact, or admin scope leak was found. The known admin
broad-test risk remains isolated and must not be reported green.

## Recovery fix round 1 independent re-review（append-only）

Reviewed fix commit: `7f0154fc1afaceacde36adf090c1760e7f376e34`

Verdict remains **FIX_REQUIRED**. The lexical generic Important is **PARTIAL**；无新 Critical或
独立 Minor。Round 1已正确覆盖 fn/impl/trait/struct/type alias/associated method generics、where/
range isolation、const generic root规则、nested generic bounds/HRTB、unknown/transitional bounds，
且 purity `47/47 + live29`。

同一 Important仍有三个开放子路径：

1. 合法 nested item shadowing + call `inner::<T>()` 因 block-local item binding未建模，被报
   `unknown path root ... inner`；inner自身shadow T合法，仍不得捕获outer T。
2. `macro_rules!` definition range虽从lexical generic index排除，但普通qualified-path第二遍仍扫描
   generic-looking macro body，未调用macro也会因 `T::Item`误杀。
3. Tokenizer的 `LIFETIME + ::` 未进入candidate parser，非法 `'a::Item` 被 ACCEPT；合法 lifetime
   bounds/HRTB必须继续通过。

Round 2最小修复必须记录function/block nested item binding range、只跳过完整macro definition而不
放松unknown macro invocation、显式拒绝lifetime root，并保留全部47项与完整recovery gates。
Task 4继续 blocked。

### Round 2 implementer response（pending independent re-review）

- `LexicalItemScope` 为nearest enclosing function/block记录nested fn/struct/type等item bindings；
  nested shadow generic与outer `inner::<T>()`通过，nested item捕获outer generic及block-end泄漏仍拒绝。
- 普通include/extern/use/qualified-path scan统一跳过 `macro_definition_ranges`完整token range；实际
  external macro与unknown module-scope invocation仍拒绝。
- `parse_candidate_path` 显式拒绝 `LIFETIME + ::`；合法 lifetime bounds/HRTB、char literal与
  single-colon loop label保持。
- Fresh RED `4`，GREEN purity `54/54 + live29`；完整recovery gates PASS。状态仅为
  `FIX_ROUND_2_IMPLEMENTED_PENDING_REREVIEW`，不预判review verdict。
