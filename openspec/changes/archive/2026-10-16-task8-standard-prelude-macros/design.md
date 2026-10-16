# Design

Add `assert`, `assert_eq`, and `vec` to the existing bare-prelude allowlist. Every invocation continues through `bare_macro_shadow`, which checks textual `macro_rules!`, lexical imports, and crate-root exports before accepting standard provenance. Keep qualified macro resolution unchanged. Exercise each new name with one accepted unshadowed source and one rejected local-shadow source, then run the complete purity self-test suite and final gate.
