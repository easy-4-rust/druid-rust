# Design

## Context

The SemVer allowlist is intentionally compact: it pins each crate/lint category by count and fingerprint. The migration is already approved there, but the report's individual findings have not been checked against a durable, source-controlled item ledger. This repository's canonical public entry remains `DataSource`; logical operations flow through standard connection and statement contracts. Filter dispatch, holder state, and similar helper APIs are retired internal surfaces.

## Goals / Non-Goals

**Goals:**

- Preserve the exact unique old method identity and baseline source location for every removed-method finding.
- Preserve duplicate occurrence counts so the item ledger totals exactly 2,578 inherent and 390 trait findings.
- Verify the item ledger against each live CI SemVer report and retain the category fingerprint check.
- Explain migration routes while keeping `DataSource` as the supported starting point.

**Non-Goals:**

- Reintroducing removed `DruidPooled*` or `FilterChain` methods.
- Claiming every removed helper has a one-to-one standard replacement.
- Closing the broader P04 Task 6 or Task 8 acceptance gates.

## Decisions

- Keep `api/rdbc-single-standard-breaking.allowlist.tsv` as the category-level SemVer authority. Add an item-level TSV beside it and have the existing verifier compare both layers to the same live report.
- Normalize CI checkout paths to repository-relative `crates/...` paths, and aggregate only identical `(crate, lint, symbol, source path, source line)` rows. Store the aggregate occurrence count rather than discarding duplicate SemVer findings.
- Assign `DruidPooledConnection`, statement, prepared/callable statement, result-set, and metadata proxy owners to their standard contract family. Assign filter dispatch, holder, defaults, and statistics listener helpers to an explicitly retired internal surface. This describes the supported migration route without claiming behavioral equivalence for every convenience method.
- Treat any count, digest, identity, or source-location drift as a failed gate requiring review and a regenerated record.

## Risks / Trade-offs

- A SemVer tool upgrade may change repeated finding counts or formatting. The verifier will fail closed; regenerate only after reviewing the new baseline output.
- The route identifies a contract family, not a promise that every old convenience method has an exact replacement. The existing behavioral method and deleted-test ledgers remain the evidence for preserved behavior.

## Migration Plan

Generate the item ledger from the completed SemVer reports, run the verifier and its tests, then commit the allowlist rationale, ledger, and evidence together. Downstream users must migrate to the canonical standard entry before a stable release.
