# P06 fault-budget evidence

The [Linux baseline run](https://github.com/easy-4-rust/druid-rust/actions/runs/39106637663) on `a96185a9f587959a5b138e1571c6232c8ca94c89` passed all workflow steps. It ran the Pool slow-close test 20 times and each of six real Admin HTTP/gRPC fault tests 10 times. The artifact `runtime-measurement-a96185a9f587959a5b138e1571c6232c8ca94c89` contains raw logs and JSON; the JSON records each log's SHA-256. Its measurement parsers and their contracts also passed in the same run.

| Scenario | Baseline p99 | Accepted CI ceiling |
| --- | ---: | ---: |
| Pool close storm | 848 µs | 10,000 µs |
| Pool shutdown | 212 µs | 50,000 µs |
| Admin duplicate batch | 14,481 µs | 250,000 µs |
| Admin sequence gap | 54,130 µs | 500,000 µs |
| Admin restart with pending batch | 288,998 µs | 2,000,000 µs |
| Admin unavailable shutdown | 111,301 µs | 500,000 µs |
| Admin remote authentication | 102,024 µs | 1,000,000 µs |
| Admin pending-ACK SIGTERM | 32,102 µs | 1,000,000 µs |

Pool peak RSS was 18,212 KiB and Admin child-process peak RSS was 21,744 KiB; each CI ceiling is 65,536 KiB. Pool queue peak equalled capacity 4, all 20 runs rejected 20 excess closes, all drained to zero remaining work, and the close worker terminated. Every Admin run emitted the expected sequence/projection/rejection/unflushed/pending-release observation. These ceilings allow CI scheduling variation while remaining below the tests' configured behavioral deadlines; they are process-level release budgets, not per-request latency claims. The baseline run did not enforce the new ceilings. A second run at the budget commit is required before closing Task 5.

The [enforced run](https://github.com/easy-4-rust/druid-rust/actions/runs/39107207211) on `0b0c3cb199211d92135b0885860e9463347a6879` passed every step with zero budget violations. Its uploaded artifact records Pool p99 close-storm/shutdown 544/139 µs, 18,236 KiB peak RSS and at least 20 queue rejections; Admin p99 whole-scenario times were 14,652 µs duplicate, 51,705 µs gap, 288,054 µs restart, 110,682 µs unavailable, 62,682 µs authentication, and 32,647 µs SIGTERM, with 21,828 KiB child peak RSS. Metrics saturation and runtime burst/shutdown budgets passed in the same run. P06 Task 5 still awaits the full cross-platform release gate and final module verdict; this Linux workflow alone does not close it.
