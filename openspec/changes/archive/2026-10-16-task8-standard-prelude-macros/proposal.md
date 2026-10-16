# Proposal: prove standard prelude macro provenance

## Why

The P04 Task 8 final purity gate currently rejects ordinary `assert!`, `assert_eq!`, and `vec!` invocations in reachable RDBC standard source. The gate already proves that its small prelude-macro allowlist has no local or imported shadow. Extend that same proof to these three standard Rust macros so final verification can distinguish real source pollution from standard-library use.

## What Changes

The final purity gate recognizes these three standard prelude macro names through its existing shadow-provenance check. Unknown macros, raw identifiers, local shadows, ambiguous imports, and forbidden runtime/product paths remain rejected.
