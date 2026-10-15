# Design

The existing integration test builds the repository to near its 64 MiB serialized projection cap, but reads `/proc/self/status` only on Linux. Add `memory-stats` as an Admin dev dependency and use its `physical_mem` reading to sample the current test process while filling. The crate maps this field to RSS on Linux/macOS and working set on Windows.

Sample at a regular insertion interval and after the final accepted entry so Linux's slower `smaps` probe does not run for every row. Track the largest observed sample. Fail if any probe returns `None` or the sampled high-water exceeds 128 MiB. Include the field name `sampled_physical_mem_bytes` in output so the result cannot be mistaken for a lifetime peak.

The current workflow already runs workspace integration tests on Linux, macOS, and Windows. No new workflow job is needed; confirming the named test appears and passes in those jobs is part of verification. `docs/verification/rdbc-first/runtime-hardening.md` will record each CI result and retain the sampling limitation.

## Risks

- Process memory includes test harness and concurrently running tests. The 128 MiB ceiling is intentionally higher than the repository projection cap and must be verified on each hosted OS.
- Sampling can miss short-lived spikes. The evidence therefore reports a sampled high-water mark and does not claim a lifetime peak RSS.
- Linux physical-memory reads may use `/proc/self/smaps` and are more expensive; sampling every 32 accepted rows keeps that cost bounded.
