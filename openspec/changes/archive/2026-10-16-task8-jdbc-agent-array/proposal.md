# JDBC Agent Array resource conformance

## Why

The canonical `Connection#create_array_of` contract exists, but the JDBC Agent adapter has no remote Array resource path. The Task 8 `java.sql.Array` adapter row remains planned despite the JDBC driver being capable of creating arrays.

## What changes

- Own JDBC `Array` handles in the Agent session and release them on explicit free, logical close, and session close.
- Add remote creation, metadata, value/range, result-set, and free operations with precise error propagation.
- Connect the Rust JDBC Agent adapter to the standard RDBC Array trait without claiming unsupported overloads.
- Add real H2 protocol and Rust consumer tests, then update the Task 8 inventory only for behavior directly proven.

## Impact

This adds an opt-in JDBC Agent resource path. Other adapters and their capability claims remain unchanged.
