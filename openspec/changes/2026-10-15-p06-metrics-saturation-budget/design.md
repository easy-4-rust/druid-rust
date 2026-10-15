# Design

Build the `queue_semantics_test` executable once. A Python runner invokes only `saturated_runtime_shutdown_drains_and_joins_every_owned_task` repeatedly, parses its existing diagnostic line, and writes JSON before rejecting a breached budget. Use Linux `ru_maxrss` for the child process peak and nearest-rank p50/p95/p99 for shutdown time. The scheduled/manual `runtime-benchmark` workflow uploads the JSON and raw run log, including on failure. Keep the existing Pool benchmark unchanged.
