# Tasks

- [x] 1. Add a failing test that the SQLx Any fallback does not advertise unverified transactions/savepoints before or after owned-actor handoff; retain the native SQLite positive assertion. RED showed `true/true` before handoff instead of `false/false`.
- [x] 2. Derive the capability descriptor from backend identity and pass targeted SQLx tests, formatter, and Clippy. SQLx unit tests 3/3, native SQLite/Toasty live contract 1/1, package formatter, and strict all-feature/all-target wrapper Clippy passed locally.
- [x] 3. Validate the OpenSpec change and relevant CI matrix, then sync, archive, commit, and push the verified result. Strict OpenSpec validation passed; GitHub Actions run [38915480504](https://github.com/easy-4-rust/druid-rust/actions/runs/38915480504) for `7a041f9` completed 44/44 jobs successfully, including the public API gate.
