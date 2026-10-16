# Callable implementation evidence (2026-10-17)

This is an implementation inventory, not a claim that Task 8 is complete. The standard contract is `druid_rdbc::statement::CallableStatement`; the only new executable adapter is `JdbcAgentCallableStatement` in `druid-wrapper`.

| Surface | Current behavior | Evidence / boundary |
| --- | --- | --- |
| Prepare and ownership | `JdbcAgentConnection::prepare_call` returns a session-owned Agent handle; explicit statement close and pooled owner close invalidate it. | `jdbc_agent_callable_live_test.rs`: H2 owner close and HSQLDB explicit close. |
| IN/OUT round trip | Indexed and named selectors, OUT SQL type/scale/type name descriptors, scalar OUT values and `was_null` pass through the Agent protocol. | H2 `ABS` and HSQLDB stored procedure live tests; `AgentServerTest` protocol tests. |
| Ordered results | One execution retains update count followed by two result sets; advancement invalidates the previous server cursor. | HSQLDB live test and `AgentServerTest.hsqldbCallableReturnsTwoOrderedResultSets`. |
| Failure | A JDBC execution error retains SQLState; the old OUT value is cleared and a later execution can succeed. Mixed query/update convenience calls fail rather than discarding results. | H2 failure/retry and HSQLDB mixed-result live tests. |
| Indexed IN setter variants | `RustValue`, `Boolean`, `Byte`, `Short`, `Int`, `Long`, `String` are accepted. | Other `PreparedInputParameter` variants return `feature_not_supported("jdbc_agent_callable_rich_indexed_input")` before execution. |
| Named IN setter variants | `Boolean`, `Byte`, `Short`, `Int`, `Long`, `String`, and `Object` without target SQL type/scale are accepted. | Other `CallableInputParameter` variants return `feature_not_supported("jdbc_agent_callable_rich_named_input")` before execution. |
| OUT object methods | The adapter materializes `RdbcObject::Scalar`, so scalar typed getters are available through the standard trait. | Native LOB, Array, Ref, RowId, SQLXML, reader/stream object returns are not implemented by this adapter; do not advertise them as supported. |

Still required before closing Task 8: resolve the approved cancel/retry semantics, test active cancellation and cleanup, review unbounded eager result collection against runtime budgets, run and record the full cross-platform CI plus API/SemVer gates, and sync/archive the change only after the acceptance conditions pass.
