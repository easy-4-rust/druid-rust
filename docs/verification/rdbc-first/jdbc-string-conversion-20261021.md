# Standard JDBC String conversion — 2026-10-21

OpenSpec fact source: `rdbc-jdbc-method-conformance/spec.md#MC-10`, under complete Rust JDBC requirement MC-08. The canonical ResultSet default getString path previously rejected non-String scalar values while BufferedRowSet already converted them. The default now performs one backend value read and reuses the existing typed-String conversion for Boolean, numeric, decimal and temporal values. SQL NULL remains NULL, Unicode remains intact, and index/label reads agree. This changes implementation bodies only; exported public signatures are unchanged.

Native string_value overrides retain their own output and errors instead of being replaced by generic conversion. Original driver failures still propagate before conversion. The Rust fallback keeps the existing typed-String UTF-8 byte-decoding policy and SQLDataException/22021 for invalid UTF-8; this does not certify every database driver's binary/String representation.

Three new contracts cover all scalar families, read counts, NULL observation, missing-label priority, invalid UTF-8, original backend failures and native override dispatch. The actual pooled SQLite diagnostic now proves numeric getString by index and label in both filtered and unfiltered paths. Successful numeric strings add their UTF-16 length once; failed reads and ordinary value/object accesses do not add String length. It no longer preserves the obsolete rejection of numeric getString as a compatibility policy.

## Actual verification

- Unified complete module command: `cargo +1.97.1 test -p druid-rdbc -p druid --locked --no-fail-fast`, terminal exit **0**: **3,677 passed, zero failed/ignored**, 242 result blocks, including independent external-consumer builds and doctests.
- Strict all-target command: `cargo +1.97.1 clippy -p druid-rdbc -p druid --locked --all-targets -- -D warnings -A clippy::pedantic`, terminal exit **0**.
- Thirteen method-evidence control tests passed. Method-ledger integrity, changed-file rustfmt checks, strict OpenSpec validation and whitespace checks passed. No predecessor red run is claimed: the approved workflow codes the coherent batch before unified module verification.
- Two getString overload identities receive PARTIAL links to the actual scalar-family contract. Existing evidence hashes are refreshed only for changed referenced files. Frozen denominator: **2,001 identities; 25 PARTIAL links; 1,976 unassessed**. Native adapter-specific formatting, complete logical-proxy conformance and real-database layers are not promoted by these links.

The reviewed ClientInfo public surface passed the subsequent native Darwin API job on source `2ed019b`: [public-api job 119019411297](https://github.com/easy-4-rust/druid-rust/actions/runs/39538079323/job/119019411297). That result certifies its reviewed surface, not the full semantic/workspace/platform scope of this later String implementation. Fresh source-matrix and SemVer closure, remaining JDBC identities and native adapter behavior remain open. ClientInfo failed-property values still need canonical ClientInfoStatus typing. Overall verdict: **NOT_READY**, not 100% JDBC completion or production readiness.

## Raw local receipts

Plain logs in `D:/druid-rust-tooling/`, bound by their actual SHA-256 bytes:

| Log | SHA-256 |
| --- | --- |
| `jdbc-string-modules-20261021.log` | `9d785e49bc4982457817a12b1df1c9b06d2d0793791af7ba41fe9759da9f6452` |
| `jdbc-string-clippy-20261021.log` | `e11978fd0c30902b9f0ff4d194648779c23573e7418c3a9e548b284d3f8241da` |
| `jdbc-string-evidence-controls-20261021.log` | `f227c2ddc4e66e0508e8c6e5d44d539cd11165c0fdbe925b85e9a8e127826f95` |

Standard reference: [Java 8 ResultSet getter conversions and getString](https://docs.oracle.com/javase/8/docs/api/java/sql/ResultSet.html).
