# Tasks

- [x] 1. Add a failing test that the SQLx Any fallback does not advertise unverified transactions/savepoints before or after owned-actor handoff; retain the native SQLite positive assertion. RED showed `true/true` before handoff instead of `false/false`.
- [x] 2. Derive the capability descriptor from backend identity and pass targeted SQLx tests, formatter, and Clippy. SQLx unit tests 3/3, native SQLite/Toasty live contract 1/1, package formatter, and strict all-feature/all-target wrapper Clippy passed locally.
- [ ] 3. Validate the OpenSpec change and relevant CI matrix, then sync, archive, commit, and push the verified result.
