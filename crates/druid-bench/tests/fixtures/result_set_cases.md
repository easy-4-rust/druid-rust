# Frozen ResultSet observations

The five class-qualified inputs reproduce every test in Java Druid 1.2.28
`DruidPooledResultSetTest` (four methods) and `DruidPooledResultSetTest2` (one
method), commit `33824c3dec1612711f9bb4e409319bcab2e4cd0e`. The unchanged
corpus SHA-256 is `991d7898de149880a7dc4a4e4dc767d1e5cfdc73f7b2444dbedc7561b8037de6`.
`result_set_java_sources.tsv` pins both original test sources and five relevant
Java implementation sources. The original test hashes are
`ee85b18a97544245482be0e2d451f9edcc440f2a79efdd51584da40d6fbf136a`
and `de82d133e82c70c4569a0a2c2c73a3bad17f45906d41dd5269c4d0dcaef5f088`.

The Java helper recompiles the two unchanged original test classes, calls each
class's original setup/teardown, verifies its stat-filter configuration, and
executes the explicit SQL, binding value, overload and navigation actions. It
retains the original native-Mock unwrap, unsupported typed-getObject, navigation,
peak-count and rowUpdated assertions. Runtime version, input digest, reflected
case identities, duplicate rows and the denominator are guarded. The production
Java classpath must be from the frozen 1.2.28 build; checking the listed source
files does not attest or rebuild every transitive production class.

The Rust test uses the real Druid pool, standard PreparedStatement and ResultSet
proxies, and the real StatFilter where configured. Its small native fixture models
only the Mock calls exercised by these five cases. It retains MockResultSet's
boundary behavior: a false next/previous does not advance its rowIndex. Its typed
getObject errors match ResultSetBase's SQLFeatureNotSupportedException with code
zero and null state/message. This is controlled pool/proxy evidence, not SQLx,
Toasty, RBDC or real-database capability certification.

Native result-set identity is measured against the actual allocation address
recorded before the owned cursor is transferred into the pool. Logical identity
uses address equality of the returned standard view. No concrete-type extension,
Any escape, unsafe dereference or fabricated native identity is used. A read-only
filter samples the real fetch-count context at completed false navigation
boundaries; the original case samples all three counts at those boundaries.
It never mutates the context, substitutes the count, or performs an extra driver
operation to obtain it.

With DRUID_JAVA_ORACLE_ROOT, DRUID_JAVA_ORACLE_JAVA_HOME,
DRUID_JAVA_ORACLE_CLASSPATH and DRUID_JAVA_ORACLE_WORK_DIR configured:

```powershell
cargo +1.97.1 test -p druid-bench --locked --test java_rust_result_set_differential pinned_java_and_rust_compare_all_five_result_set_inputs -- --ignored --exact --nocapture
```

The runner saves runtime versions, Git head/dirty status, source/corpus/oracle
hashes, diagnostics, inputs and both 15-column observation tables. The comparison
has 30 named columns and retains every input. Default tests validate complete Rust
observations and cleanup; they do not substitute for the explicit live comparison.

Current live outcome: three MATCH and two MISMATCH. The two wrap cases differ
only in raw/logical unwrap identity. No allowance exists: the explicit live test
returns exit 101 until compatibility is implemented or an exact acceptance change
is explicitly approved. The existing Statement completion allowance does not
apply to ResultSet. The full per-case records are under
`docs/verification/rdbc-first/resultset-five-20261020/`.
