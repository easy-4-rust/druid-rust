# Linux runtime budgets at 3279d31

Run: https://github.com/easy-4-rust/druid-rust/actions/runs/39462981930

Source: 3279d3155c044fd1ed6031761198086c65230755. Workflow: runtime-benchmark, Ubuntu 24.04 / Rust 1.97.1. Artifact 12284266940, runtime-measurement-3279d3155c044fd1ed6031761198086c65230755. Result: SUCCESS; all six reports have zero budget violations. The retained JSON files are the original plain measurement records; receipt.tsv gives exact byte hashes. Original logs and Criterion samples are retained locally at D:/druid-rust-tooling/runtime-3279d31-20261021 and in the CI artifact. Every declared sample/run-log digest was checked against those original artifact files, and numeric results were independently compared to the unchanged limits.

| Scope | Observed | Limit |
| --- | --- | --- |
| Pool burst p99 / shutdown | 25.509 ms / 1 ms | 100 ms / 250 ms |
| Pool active peak / maximum wait queue | 8 / 56 | 8 / 56 |
| SQLite cursor first-row / full-consume p99 | 509.493 us / 44,861.766 us | 2,000 us / 200,000 us |
| SQLite full-consume throughput | 24,519.8 rows/s | at least 5,000 rows/s |
| Pool/FilterChain/Wall/Adapter microbenchmarks | all 11 cases within individual p99 limits | unchanged per-case limits in JSON |
| Metrics saturated shutdown p99 | 129 us; 20 runs | 100,000 us |
| Pool slow-close storm / shutdown p99 | 710 us / 197 us; 20 runs | 10,000 us / 50,000 us |
| Admin network faults | all 6 scenarios within limits; 10 runs each | unchanged per-scenario limits in JSON |

Each report's RSS observation is below 65,536 KiB; the largest reported value is 25,484 KiB. RSS scopes follow the reports (including maximum-of-child-processes for Admin), not a newly inferred sum of process trees. This evidence is specific to the measured Linux runner and source; it does not certify all platforms, database versions or the unresolved API/behavioral differences.
