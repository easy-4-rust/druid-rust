# Metrics and Admin runtime/security verdict

Source `078dfa2`, 2026-10-18. The [44/44-success driver matrix](https://github.com/easy-4-rust/druid-rust/actions/runs/39112028023) includes an Ubuntu `rust-contracts` job that passed both default and all-feature workspace test suites. These suites contain the listed protocol, network, repository, shutdown, and security test files. The separate [runtime budget run](https://github.com/easy-4-rust/druid-rust/actions/runs/39112049201) completed successfully with zero parser or numeric-budget violations.

| Contract | Current evidence | Verdict |
| --- | --- | --- |
| One Metrics protocol | `crates/druid-metrics/proto/druid_metrics_v1.proto` is the only `.proto` source under `crates/`; all-feature job log records `protocol_roundtrip_test` 14/14 and `shared_protocol_compile_test` 1/1. | PASS for protocol topology and suite |
| Real network delivery, replay and shutdown | All-feature job log records `grpc_network_e2e_test` 50/50, `push_worker_test` 6/6, `shutdown_test` 8/8 and `repository_capacity_test` 10/10. The runtime artifact records ten repeated real Admin scenarios per fault type, 20 Metrics saturation runs and zero unflushed/active work at closure. | PASS for tested paths |
| Pool, Metrics and Admin numeric budgets | Runtime burst p99 25.532 ms and 19,060 KiB RSS; Pool 20-run storm/shutdown p99 1,268/291 µs and 18,264 KiB RSS; Metrics 20-run shutdown p99 149 µs and 17,996 KiB RSS. Admin ten-run p99 duplicate/gap/restart/unavailable/authentication/SIGTERM were 16,167/53,696/289,548/112,245/65,802/33,358 µs, with 21,860 KiB child RSS. All configured ceilings passed. | PASS on Linux budget runner |
| Admin authentication, TLS, redaction | `admin_binary_test` contains default loopback, non-loopback TLS+auth rejection, protected HTTP/gRPC, invalid TLS material and secure-cookie cases; all-feature log records 18/18. `admin_trace_redaction_test` checks captured diagnostics, 1/1. The plan's former `security_e2e_test` filename did not exist and has been corrected to these actual suites. | PASS for implemented test cases; broader security audit not claimed |

The test counts above come from the completed `rust-contracts` job log. P06 Task 5's cross-platform release condition is now satisfied by the completed matrix; broader P07 semantic/performance acceptance remains separate.
