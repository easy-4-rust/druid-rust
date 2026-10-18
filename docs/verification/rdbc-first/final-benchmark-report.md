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

The Windows runtime p99 is 126.034 ms, below the documented 150 ms local reference ceiling. The [final-head Linux release workflow](https://github.com/easy-4-rust/druid-rust/actions/runs/39112049201) enforced runtime, Pool slow-close, Metrics saturation, and Admin fault budgets successfully. Other microbenchmark cases have measured values but no accepted cross-platform numeric ceilings; these results alone do not establish a performance regression gate or release readiness.

## SQLx SQLite cursor follow-up

The `adapter` target now also measures a real forward-only SQLx SQLite cursor over a 1,000-row recursive query. Both cases reuse one physical connection and create/close a new statement and cursor for every iteration. `first_row` times query admission through the first successful `next` and cursor cleanup; `full_consume` times all 1,000 successful rows and final exhaustion/cleanup. The same Windows host and Rust toolchain ran 100 Criterion samples for each case; the complete adapter target exited 0.

| Case | p50 µs | p95 µs | p99 µs | Sample SHA-256 |
| --- | ---: | ---: | ---: | --- |
| adapter_sqlite_cursor_first_row_1000 | 207.25 | 218.05 | 224.05 | `00acb2ac8d3f794ef404ec33c17269876295b9dd9704e563d495799568beae5d` |
| adapter_sqlite_cursor_full_consume_1000 | 3,524.66 | 4,292.89 | 4,770.35 | `d8df9c0a6f492810a02b0a310a233a3dae2e5206289edbc5788e34922367894b` |

The median full-consume rate derived from this batch is about 283,700 rows/s, including statement/query setup and cleanup. This is a measured Windows baseline, not an accepted regression ceiling; per-case allocation and RSS were unmeasured in this batch.

The `85fed76` Windows validation reran only these two cases while other Rust builds were active on the host. The same benchmark source and 100-sample Criterion path produced first-row p50/p95/p99 **214.513/272.508/389.304 µs** (sample SHA-256 `507415ae8791c465d6fd64314139534e08b7d86c77e516d431e06a19bcd4a9a2`) and full-consume **29,422.544/31,274.156/32,056.850 µs** (SHA-256 `cf5f92010a11b645a0bdbf0b836ebfc7b8a4acab7744f00962f53149a80979b5`). The command exited 0 and confirmed the two-case filter. This large difference is unresolved host-level measurement variability, not proof of a source regression or an acceptable ceiling.

The dedicated [Linux baseline run 39116529320](https://github.com/easy-4-rust/druid-rust/actions/runs/39116529320) on `85fed769ca59ff29dbdcf4764fe4e63942a3d92e` completed successfully. Its 100-sample first-row p50/p95/p99 was **475.861/499.489/514.844 µs** (sample SHA-256 `e72dcb80eee908b4fdd77a42b4d6e9a8b5aa41ccc04c9cbdebb53085c3bf1ace`); full-consume was **42,187.768/45,237.757/46,263.592 µs** (SHA-256 `fa009e4cc8a7ce619089682865bcdc392f4906082822d1bb493e46049e090104`). Median throughput was **23,703.6 rows/s** including statement/query setup and cleanup; benchmark-child peak RSS was **26,224 KiB**. The uploaded measurement JSON has SHA-256 `06af4ed9809ca6cd3a42deba6bcde5b125a33ad6bfde530ce6f54bb0a2d6d44d`; its status is explicitly `BASELINE_ONLY`.

The release-runner budget uses approximately four times the baseline p99 latency as scheduling headroom: first row ≤ **2,000 µs**, 1,000-row full consume ≤ **200,000 µs**, throughput ≥ **5,000 rows/s**, and peak RSS ≤ **65,536 KiB**. The throughput floor checks the derived-rate field independently. These are process-level regression guardrails, not application query SLOs; they apply to this SQLx SQLite case on the Linux CI runner.

The [enforced Linux run 39117292451](https://github.com/easy-4-rust/druid-rust/actions/runs/39117292451) on `bd3855b890b8316943829444db0fe58e807e4298` completed successfully. Its 100-sample first-row p50/p95/p99 was **469.795/487.139/500.016 µs** (sample SHA-256 `2983d36d15c01d6c3acd9456801de066d6b640823c2b59b1503437ac5639cf0e`); full-consume was **40,563.586/43,005.731/43,881.207 µs** (SHA-256 `e7feee16f35fbb4544c6c4f3747fea5664d58a481151acb3b159382819b93986`). Median throughput was **24,652.7 rows/s**; benchmark-child peak RSS was **23,676 KiB**. The uploaded measurement JSON has SHA-256 `952dc6ac8bcf29ec6b830fb8d03943fb538b33d84446ca86c1db74212fb7d6d3`, status `ENFORCED`, and zero violations. This closes the SQLx SQLite cursor regression gate; other microbenchmark cases still lack accepted numeric ceilings.
