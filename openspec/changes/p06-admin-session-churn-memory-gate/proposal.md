# Admin session churn memory gate

## Why

The Admin repository has TTL, service and serialized-state limits, and a cross-platform 128 MiB sampled-memory check for a single fill. P06 Task 5 still lacks a process-memory budget while expired service sessions are replaced repeatedly. A bounded latest-state map alone does not prove process memory stays bounded under churn.

## What Changes

- Add an isolated cross-platform test that runs 50 generations of 32 service identities with two datasource projections each, each carrying a 60 KiB SQL projection, separated by TTL expiry.
- Sample RSS on Unix and working set on Windows throughout the run. Fail if the reading is unavailable or the sampled high-water mark exceeds the existing 128 MiB Admin repository ceiling.
- Assert each generation retains only the current 64 projections, prior session identities disappear, 3,136 entries are TTL-evicted, and no capacity rejection occurs. Record latency percentiles as observations, without imposing a host-dependent latency threshold in this change.
- Run the test in normal CI and record measured results. P06 Task 5 and overall production readiness remain open for the other fault and latency budgets.

## Impact

The change adds verification only; it does not alter the repository's runtime policy or public API. A failing gate will expose a regression in TTL reclamation or process memory growth before release.
