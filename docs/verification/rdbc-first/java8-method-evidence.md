# Java 8 public-API evidence ledger

The frozen inventory has 2,001 Java public identities, including methods, inherited methods and constants. Its denominator is extracted from OpenJDK Java 8 source, not inferred from Rust APIs. An unassessed identity is an evidence gap; it is not proof that the implementation is missing.

## Integrity and coverage are separate

```sh
python scripts/verify_java8_method_evidence.py
python scripts/verify_java8_method_evidence.py --require-complete
```

The default command checks the frozen inventory identity/hash, exact ledger denominator, source/test hashes, mapped method presence, passed-layer evidence and precise unsupported claims. Every linked row must point to an existing, unique OpenSpec requirement ID, written as change-name/spec.md#ID-NN. Missing paths, unknown IDs, duplicate requirement IDs and paths escaping openspec/specs are rejected. PARTIAL claims need an actual mapped method, positive test reference and passed evidence layer, just as SUPPORTED claims do.

Successful default output is EVIDENCE_INTEGRITY_PASS. It does not certify the inventory's complete behavior or production readiness. The optional --require-complete diagnostic also returns nonzero while any identity has partial/unassessed capability, unassessed/not-run/failed layers, or otherwise incomplete coverage. This option exposes completeness of this ledger; it does not change the approved release scope or silently add a new release requirement.

## Current bounded evidence

On 2026-10-21, 17 identities have PARTIAL evidence links and 1,984 remain UNASSESSED. The eight added links are Prepared/Callable native executeQuery, generic execute and close (including inherited Java identities), plus ResultSet next and close. They refer to the executed native-proxy regression for query/generic cursor SQL attribution, consumption and once-only close accounting. Other overloads, errors, standard-contract completeness, backend matrices and real database layers remain unassessed in these rows. Separate live H2 receipts are retained in wall-table-20261021 and are not automatically promoted to every layer of these entries.

The previous nine links now use actual requirement IDs; the stale rdbc-xa-resource path is corrected to rdbc-xa-resource-contract. Byte hashes for linked Rust sources and tests use explicit LF checkout attributes so platform line endings cannot invalidate the evidence. Existing evidence statuses are preserved.

Mutation tests cover missing/escaping/ambiguous specifications, unknown requirements, partial claims without mappings, tests or passed layers, contradictory unassessed states, and the distinction between integrity and optional completeness checking. No ledger row is marked fully SUPPORTED by this increment.


## Canonical inventory bytes

The tracked LF inventory digest is 1FACDC7C75308DB0AF9B023847B82B96594D75B667D694774439ED474E53A257. Earlier archived records used the Windows CRLF byte digest 27F424D0FC4262D8862F93044BD61D35A5BAB3FD85511D58D5CCE5DEB7FFE4F5. The 2,001 identities and pinned Java commit are identical in both views. The generator now canonicalizes the doclet's platform line separators before atomically publishing output; the ledger records the tracked LF digest. Verification remains byte-exact and rejects the stale platform-specific digest. The historical archive is preserved as history. Thirteen verifier/generator controls, the eleven-test tool module and an actual JDK 8 regeneration passed; regenerated output matches the frozen Git bytes exactly. See inventory-lf-20261021.tsv.
