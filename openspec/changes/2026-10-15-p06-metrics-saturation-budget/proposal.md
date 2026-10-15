# Metrics saturation budget evidence

The existing saturated runtime test proves clean shutdown but only prints one timing. P06 Task 5 needs repeatable numeric evidence and a failing CI budget for this fault case.

Measure repeated executions of the existing real-runtime test on the scheduled/manual runtime workflow. Preserve each run's counters, nearest-rank shutdown percentiles, process peak RSS, and explicit budget violations in an artifact. The test and measurement must fail on missing output, a nonzero exit, no coalescing, or remaining owned work. This change does not close the other P06 fault cases.
