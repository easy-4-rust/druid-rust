# P06 Admin cross-platform repository memory budget

The Admin repository already enforces a 64 MiB serialized-state cap and checks process memory on Linux. The RSS assertion is compiled only on Linux, leaving macOS and Windows CI without evidence for the same bounded-churn workload.

This change measures process physical memory while filling the repository near its configured state cap, applies the existing 128 MiB ceiling on Linux/macOS/Windows, and records the metric name and platform semantics precisely. The test reports a sampled high-water mark; it does not claim the operating system's lifetime peak RSS.

## Scope

- Keep repository limits, workload size, and admission behavior unchanged.
- Add a test-only cross-platform physical-memory probe.
- Execute the memory budget test in the existing platform CI matrix.
- Update the runtime hardening evidence with exact platform runs and limitations.

## Out of scope

- Changing the 64 MiB repository state budget or 128 MiB process-memory ceiling.
- Claiming a production RSS guarantee from one test process or one CI run.
- Adding runtime memory sampling to the Admin service.
