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
