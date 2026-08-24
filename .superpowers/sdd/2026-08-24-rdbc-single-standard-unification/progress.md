# SDD ledger — plan: docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md

BASE: bb91193d9bb488b107b8c9ac6dd56f8756ed9593
SPEC: docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md
Ruling: execute on authorized `main` without a worktree because repository AGENTS.md forbids worktrees; if wrong, shared-checkout coordination risk increases, but branch/push/publish authority remains unchanged.

## Preflight task/interface scan

| Task/interface pair | Produces -> consumes | Finding |
| :--- | :--- | :--- |
| Task 1 self | RED live error denominator/tests -> final RdbcError/DruidError + six named gates | CONSISTENT; Task 1 is mandatory and no later task may start without all six PASS. |
| Task 2 self | OpenJDK semantic rows + P03 standardization input -> planned target/consumer manifests | CONSISTENT; planned implementation states are explicitly allowed only in Task 2. |
| Task 3 self | planned targets -> pure independent standard traits + ownership compile contracts | CONSISTENT; temporary Physical remains non-canonical and no alias fakes identity. |
| Task 4 self | standard traits -> wrapper dual impl/single core + adapter-live evidence | CONSISTENT; all-features gates cover optional families. |
| Task 5 self | wrapper standard impls -> holder `Box<dyn Connection>` + pooled/logical impl + Druid internalization | CONSISTENT; ownership/metadata/resource policies are explicitly tested. |
| Task 6 self | all consumers switched -> zero Physical names/spi + final standard purity | CONSISTENT; tracked consumer manifest is the dynamic file denominator. |
| Task 7 self | clean standard owner -> resource/capability/async ResultSet hardening | CONSISTENT; Task 3 already freezes final async ResultSet signature. |
| Task 8 self | Tasks 1–7 artifacts -> quality/API/SemVer/differential/real-DB completion audit | CONSISTENT; missing cargo-semver-checks stops only Task 8 and cannot be reported PASS. |
| Task 1 -> Task 2 | final RdbcError and product error boundary -> inventory error adapter | CONSISTENT. |
| Task 2 -> Task 3 | semantic inventory + standardization/consumer denominators -> target modules/signatures | CONSISTENT. |
| Task 3 -> Task 4 | object-safe owned-handle standard traits -> adapter implementations | CONSISTENT. |
| Task 4 -> Task 5 | adapter-live standard Connection/Statement/Metadata/Resource -> Druid holder/proxies | CONSISTENT. |
| Task 5 -> Task 6 | logical-live + internalized Druid runtime -> Physical deletion | CONSISTENT. |
| Task 6 -> Task 7 | no-Physical public surface -> unified runtime hardening | CONSISTENT. |
| Task 7 -> Task 8 | shared conformance/runtime evidence -> release-style completion audit | CONSISTENT. |
| Tasks 2/3/4/5/8 shared `jdbc42-semantic-inventory.tsv` | PLANNED_TARGET -> DECLARED -> adapter LIVE -> logical LIVE -> final evidence | CONSISTENT; each phase has a distinct verifier mode. |
| Tasks 2/4/5/6 shared `rdbc-production-consumers.tsv` | emitted live occurrences -> target_task rows consumed -> final zero transitional consumers | CONSISTENT; fixed file counts cannot replace manifest equality. |
| Tasks 1/4/5/6/7 shared crate sources | error signature -> standard impl -> holder cutover -> old-name deletion -> behavior hardening | CONSISTENT ordering; no task builds on deleted Physical paths before cutover. |
Task 1 minor (deferred): `compat/exceptions.rs` still documents Java narrowing as saturation; final review must decide whether wording blocks merge.
Task 1 minor (deferred): `.superpowers/sdd/.../task-1-report.md` was force-tracked despite the SDD workspace ignore rule and plan staging scope; final review must decide whether to remove it from tracked history.
Task 1: fix round 1/5 complete (3 Important fixed with RED/GREEN: real batch paths retain i64 counts; product fatal errors cross Pool through structured class/message/source; wrapped batch accessor delegates all three views; base commit 0d83f22 reviewed; 2 Minor remain deferred above)
Task 1 ledger correction: round 1 was not complete because scoped re-review found a new Important in the fix diff; the prior completion wording is superseded by this line.
Task 1: fix round 1/5 (3 original findings addressed, 1 new Important open — grouped export verifier accepts aliased/underscore RdbcError and Gate 2 is not fail-closed; commits 0d83f22..7436321)
Task 1: fix round 2/5 in progress (1 Important open: canonical RdbcError export parser must reject aliases/underscore/child paths and handle nested grouped use)
Task 1: fix round 2/5 (1 addressed, 0 open — canonical export verifier fail-closed; commits 7436321..d25ef68)
Task 1: complete (commits bb91193..d25ef68, review clean after 2 fix rounds; 2 deferred Minor findings)
Task 2 minor (deferred): `.superpowers/sdd/.../task-2-report.md` was force-tracked outside the brief's ten authorized files; final review must decide whether to remove it from tracked history or formalize report archival.
Task 2: fix round 1/5 in progress (1 Critical + 2 Important open: relative/receiver consumers omitted; unrelated same-leaf fallback pollutes denominator; JSON/TSV exact-schema/canonical bytes are not fail-closed; commit 317e599 reviewed)
Task 2 minor (deferred): task-2 report retains round-0 3145/hash table before appended round-1 3661 data; final review must decide whether explicit superseded labeling is needed.
Task 2: fix round 1/5 (same-leaf and JSON/TSV findings addressed; 1 Critical remains — file-global receiver name map misclassifies scoped/shadowed receivers; 1 new Important — cfg(test) exclusion missing from public-surface/trait-method derivation; commits 317e599..4628c17)
Task 2: fix round 2/5 in progress (scope-aware receiver declaration identity + shared production cfg attribute-range model)
Task 2: fix round 2/5 (0 addressed, 2 open — generic impl self type/accessor identity and self.field/bare/closure/pattern scopes still fail; cfg comma item ranges/double-not/cfg_attr still fail; commits 4628c17..b542fab)
Task 2: fix round 3/5 in progress (last resume of original implementer: full impl self-type parsing + receiver expression categories + lexical patterns + cfg test=false satisfiability/item boundaries)
Task 2: fix round 3/5 (0 addressed, 2 open — qualified generic/associated self identity, nested-comma closure and match guard still fail; nested cfg_attr and comparison `<` node boundaries still fail; commits b542fab..b2962fa)
Task 2: fix round 4/5 in progress (fresh high-capability implementer after three prior attempts; owns the generalized parser findings)
Task 2: fix round 4/5 (cfg finding addressed; receiver Critical remains — equivalent super/crate self types and turbofish-comma closure scope fail; commits b2962fa..5f4874f)
Task 2: fix round 5/5 in progress (fresh final implementer; breaker will adjudicate any residual)
Task 2: fix round 5/5 (closure turbofish finding addressed; 1 Critical remains — module-context identity over-collapses child local vs crate-root and external-prelude vs crate-module; commits 5f4874f..da3ba90)
Task 2 Ruling: reviewer is correct; the residual is real and load-bearing. Before Task 3 creates any standard trait, it must normalize all repository-relative self types to a full `<current-crate>::<module>::Type` identity, while bare/external-prelude paths without source/re-export proof remain an unresolved namespace and never merge with `crate::...`; add local-vs-root and external-vs-crate-module RED/GREEN fixtures, regenerate the consumer manifest, and re-run all Task 2 gates. If wrong, Task 3 can build canonical traits from a denominator that silently merges distinct impl owners.
Task 2: complete (commits d25ef68..da3ba90, breaker after 5 rounds; 1 load-bearing ruling carried into Task 3, 2 deferred Minor findings)
Task 3 minor (deferred): `rdbc_error.rs` identical-arm merge is behavior-equivalent but outside the Task 3 brief; final review must decide whether scope hygiene requires separation.
Task 3: fix round 1/5 in progress (2 Critical + 5 Important open: Wrapper canonical unwrap/error semantics; truthful capabilities; credential/close defaults; 1-based/generated keys; complete metadata enums; dynamic purity tree; stale Task1 external consumer fixture; commit 26fdcab reviewed)
Task 3: fix round 1/5 (7 findings addressed with strict RED/GREEN, 0 reviewer findings open; Task2 absolute identity and 3559-row consumer bytes retained; workspace all-feature check PASS; all Druid/RDBC tests PASS; non-RDBC admin supervisor exact test remains a reproducible external risk and was not modified)
