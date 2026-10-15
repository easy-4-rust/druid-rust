# Expose dynamic routing through the canonical data-source contract

## Why

`DynamicRdbcDataSource` already routes product connection requests across switchable groups, but it does not implement the unified RDBC `DataSource` contract. Standard consumers therefore cannot use dynamic routing through the same interface as ordinary and native sources.

## What Changes

- Implement the canonical `DataSource` and `CommonDataSource` contracts on the existing dynamic source without constructing another pool.
- Expose same-object canonical unwrap views; keep unsupported per-call credentials at SQLState `0A000`.
- Verify standard acquisition before and after an atomic group switch and holder return to the correct pool.

## Capabilities

### Modified Capabilities

- `rdbc-common-data-source`: extend canonical acquisition and inspection to dynamic routing sources.

## Impact

Additive trait implementations on `DynamicRdbcDataSource` and one external-consumer test. Existing product routing remains unchanged.
