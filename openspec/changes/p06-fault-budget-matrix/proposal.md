# P06 fault and operating budget matrix

The runtime hardening plan still lacks comparable numeric evidence for slow physical close and Admin transport faults. Existing tests assert bounded behavior, but their successful output does not produce a repeatable release record. Add machine-readable fault measurements and CI ownership without changing the pool, Metrics, or Admin public APIs.

This change covers P06 Task 5 only. Preserve all existing behavior assertions. First implement measurement emission, parsing and workflow wiring; after the coding batch, measure baselines and accept numeric limits using the full module test phase. Keep unrelated P06 and P07 work open until separately proven.
