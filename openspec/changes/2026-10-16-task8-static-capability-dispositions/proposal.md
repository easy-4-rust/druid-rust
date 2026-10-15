# Classify static JDBC declarations without runtime capabilities

## Why

The Task 8 inventory currently labels value objects, exception kinds, enums, and the `Types` module as `PLANNED` in `capability_state`. These declarations have no adapter capability switch to implement. Leaving them planned obscures the genuinely unfinished resource and connection behavior rows.

## What Changes

- Classify only static DTO, enum-kind, and module rows without adapter or logical implementations as `NOT_APPLICABLE` for runtime capability evidence. Keep `DriverManager` pending because it maps to an active registry service.
- Preserve the frozen OpenJDK source digest by normalizing this evidence-only classification during phase verification.
- Fail closed if a behavioral row or active implementation is labeled as statically not applicable.

## Impact

This updates the Task 8 semantic inventory and its verifier. It does not claim that adapter, logical, or real-database acceptance is complete.
