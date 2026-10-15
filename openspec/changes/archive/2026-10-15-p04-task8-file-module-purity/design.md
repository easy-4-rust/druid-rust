# Design

`ModuleScopeIndex` already resolves the public module declaration to its real source. Change `discover_standard_files` to enumerate a canonical root's `module.rs` when present, plus any physical child files in `module/`; keep the existing `module/mod.rs` + object-file path for directory roots. Fail if both root forms exist. The resulting path list continues through the unchanged token, path, and identity checks. Self-tests use the same temporary crate fixture and prove accepted, polluted, and ambiguous cases before running the real repository gate.
