# Expose canonical data-source views from the wrapper

## Why

`DruidRdbcDataSource` implements the canonical `DataSource` and `CommonDataSource` traits, but its `Wrapper` hook only reports the `Wrapper` view. Standard consumers cannot unwrap the interfaces this object actually implements.

## What Changes

- Return same-object canonical `DataSource` and `CommonDataSource` views from `DruidRdbcDataSource`.
- Preserve existing credential checks, pool identity, and unsupported-target behavior.

## Capabilities

### Modified Capabilities

- `rdbc-common-data-source`: require wrapper inspection and unwrapping to expose the implemented standard views.

## Impact

One wrapper implementation and an external-consumer regression test; no new pool or public type.
