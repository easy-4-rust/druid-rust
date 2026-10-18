# P07 benchmark batch

On 2026-10-18, Windows NT 10.0.26200.0, Rust/Cargo 1.97.1, source `16c71fc623b7b63d56b9c90c677b78cd5b290b70`, the five `druid-bench` targets `pool`, `filter_chain`, `wall`, `adapter`, and `runtime` each completed `cargo bench -p druid-bench --locked --bench <target> -- --noplot` with exit code 0. Cargo build and temporary directories were on D:; Criterion samples remain under `D:\druid-rust-tooling\target\criterion\<case>\new\sample.json`. The table uses 100 samples per case, `times[i] / iters[i]`, and nearest-rank percentiles. Values are microseconds per iteration.

| Case | p50 | p95 | p99 | Sample SHA-256 |
| --- | ---: | ---: | ---: | --- |
| adapter_parallel_64_borrow | 86.12 | 106.00 | 126.00 | `cb98d4bf5070a13844ab34ae3ece7a0a7a09bca1c4f3e6e99af1a5fc4b4d1411` |
| adapter_parallel_8_borrow | 9.65 | 11.74 | 12.81 | `0d96b77a990e5fbe839ddcd48950b4b89818e772d1d361dec7160798648cdaa4` |
| adapter_single_borrow | 1.59 | 2.04 | 2.18 | `c3843aa5c126a67bc5af6c0e0d80ce84213682014d91349139df8cf19ee51cbe` |
| filter_chain_20_adapter_build | 2.48 | 3.24 | 3.62 | `7c135040a7c59a5fa1148c2f6c9c01a371368377f6e5becee23d7ac38ad73766` |
| filter_chain_5_adapter_build | 0.60 | 0.83 | 0.89 | `2689c8c7a80320f1859a52b14cabde9803599b60b97e084689232fb91df79b40` |
| filter_chain_empty_build | 0.00 | 0.00 | 0.01 | `607bd37175a9d0fb9e50e46be48c0ba08d227e1917f13a581df083ac9d618f7a` |
| pool_parallel_16 | 20.54 | 25.02 | 26.55 | `fe02bc03468efa9ce693d3816acd4e60dee2b4cc5aa69116949061d4b1c50f2a` |
| pool_parallel_64 | 95.34 | 110.20 | 120.14 | `c433d035ef91b2cdb3158e65b444fb48148e2380dd400777070973240cc0d2e2` |
| pool_single_borrow_return | 1.38 | 1.61 | 1.82 | `2f020839194258532b994d1f167b59ae62ad2632eae161b0ff17d67094d6b31d` |
| runtime_pool_burst_64 | 123,838.20 | 125,697.80 | 126,034.10 | `536c5f4ed21287605d68cc4229dc904acf47f049b283fdc659035aa1266a6c41` |
| wall_check_2_safe | 0.73 | 0.88 | 0.94 | `488410de1c90177fa09977f7a1b6c520d905fed61ee1889fe356eed95c07b95b` |
| wall_check_all_categories | 2.25 | 2.76 | 3.19 | `b763b0f00ac80ac0053a8b194a4045b50a1f9649bbe9942a7b67fe69aebeef13` |

The Windows runtime p99 is 126.034 ms, below the documented 150 ms local reference ceiling. The separate [Linux release workflow](https://github.com/easy-4-rust/druid-rust/actions/runs/39107207211) enforced runtime, Pool slow-close, Metrics saturation, and Admin fault budgets successfully. Other microbenchmark cases have measured values but no accepted cross-platform numeric ceilings; these results alone do not establish a performance regression gate or release readiness.
