# Tasks

- [x] 1. Add failing verifier tests for unshadowed `assert!`, `assert_eq!`, and `vec!`; keep local-shadow rejection for each. All three positive controls failed before implementation; local and imported shadows have explicit rejection checks.
- [x] 2. Extend only the proven prelude allowlist, run the targeted tests and complete purity verifier suite. The targeted test and all 117 purity verifier tests passed with UTF-8 subprocess decoding.
- [x] 3. Run `verify_rdbc_single_standard_purity.py --phase final`, record remaining failures or success, validate OpenSpec, commit and push the change. The final gate passed with `STANDARD_FILES=129` and `PURITY_PHASE=final`; strict OpenSpec validation and the repository commit/push follow this task update.
