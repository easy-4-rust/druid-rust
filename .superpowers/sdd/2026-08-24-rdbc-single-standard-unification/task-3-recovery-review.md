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
