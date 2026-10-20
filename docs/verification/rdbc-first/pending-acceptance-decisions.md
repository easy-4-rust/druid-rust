# Pending acceptance decisions — 2026-10-21

Status: **AWAITING_USER_DECISION**. This is a review index of existing observations, not an approved specification or an allowance file. Existing live comparators and release gates continue to fail on the differences below. Previously submitted confirmation requests remain pending; an answer applies only to its stated scope.

## Behavioral observations

| Group | Exact cases | Observed difference: Java / Rust | Evidence |
| --- | --- | --- | --- |
| D01 — ResultSet identity, 2 cases | DruidPooledResultSetTest.testWrap; DruidPooledResultSetTest2.testWrap | raw=true, logical=false / raw=false, logical=true. Rust exposes the governed logical ResultSet view. | [Both observations](resultset-five-20261020/comparison.tsv) |
| D02 — Pool credentials, 2 of 3 Pool cases | DruidDataSourceTest_getConnection.test_conn_user_error; DruidDataSourceTest_getConnection.test_conn_password_error | UnsupportedOperationException / SQLFeatureNotSupportedException, code 0, SQLState 0A000 and the nullable-credentials operation message. Both sides reject before borrowing/creating a connection. | [Pool observations](pool-six-20261020/comparison.tsv) |
| D02 — Pool recycle, remaining 1 Pool case | DruidDataSourceTest_recycle2.test_recycle_error | Statement.isClosed=false / true after rollback-error cleanup. Rust's logical statement follows its closed lease. | [Pool observations](pool-six-20261020/comparison.tsv) |
| D03 — Callable typed getters, 2 of 6 newly submitted cases | DruidPooledCallableStatementTest.test_getObject; DruidPooledCallableStatementTest.test_getObject_1 | errors_after=0 / 1; SQL exception payload fields match. Rust accounts for the typed getter's SQL failure. | [Callable observations](callable-seven-20261020/comparison.tsv) |
| D03 — Callable identities, 3 of those 6 cases | DruidPooledCallableStatementTest.test_wrap; test_wrap_1; test_wrap_2 | Java exposes the raw Prepared/Callable roles; Rust exposes governed logical roles. | [Callable observations](callable-seven-20261020/comparison.tsv) |
| D03 — Prepared diagnostics, remaining 1 of those 6 cases | DruidPooledPreparedStatementTest.test_hitCount | Java statement hit counts 0/1 and non-null key / NOT_EXPOSED for all three Rust fields. Property restoration, traversals and separate pool-level hit count match. | [Prepared observations](prepared-cache-20261020/comparison.tsv) |
| D04 — Vendor sorter null message, 3 cases outside the 59-case selection | oceanbase_oracle_null_message; oracle_null_message; phoenix_null_message | throws:java.lang.NullPointerException / false. | [Semantic record](final-semantic-report.md); raw corpus comparison retained at D:/druid-rust-tooling/exception-sorters-38636-1792489791863716200/comparison.tsv |

D01, D02, D03 and D04 correspond to the previously submitted decision scopes. D03 groups exactly the newly submitted six cases; it does not approve D01 or D02 by implication. The existing pre-release removal ledger already records DruidPooledPreparedStatement::key, while this behavioral comparison remains unresolved; removal approval has not been treated as a new behavioral waiver.

The fixed original 59-case selection contains ten mismatches: one D01 case, three D02 cases and six D03 cases. DruidPooledResultSetTest2 adds one identity case outside that selection, and the three D04 cases belong to the independent 576-case sorter corpus. There are **14 distinct behavioral cases pending across these scopes**, not fourteen failures inside the 59-case denominator. The 59-case partition remains 47 MATCH, 2 approved standard differences and 10 MISMATCH.

## Public API decision

D05 is the pending exhaustive-enum addition of StatementPropertyEvent::GetResultSet, reported through exactly two public export paths:

- druid::core::filter::StatementPropertyEvent::GetResultSet
- druid::core::StatementPropertyEvent::GetResultSet

Consumers matching the previously exhaustive enum may need to add the new arm. The current-source SemVer replay still fails with unlisted: druid/enum_variant_added; no enum allowance has been added. [CI report receipt for 5bb051f](semver-5bb051f-20261021.tsv) preserves the baseline, run/artifact identity and report hashes. A passing public-API snapshot comparison does not resolve this SemVer decision.

## Previously approved scope

Only the two closeOnCompletion/isCloseOnCompletion Statement differences are approved in the 59-case behavioral comparator. Their approval does not extend to any case above. The [frozen manifest](java-baseline-59-cases.tsv), comparisons and OpenSpec acceptance remain authoritative until a directly applicable human decision is recorded.
