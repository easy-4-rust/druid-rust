# Canonical DriverManager ownership

## Why

`druid::sql::DriverManager` currently owns the process-wide JDBC-style driver registry, login timeout, and diagnostic writer. The RDBC standard layer exposes `Driver` and `DriverRegistry` but no standard `DriverManager`, leaving a cross-layer contract and the `java.sql.DriverManager` semantic inventory row unfinished.

## What Changes

Move the one process-wide manager implementation into `druid-rdbc::driver::DriverManager`. Retain the existing `druid::sql::DriverManager` path as an exact re-export, so both paths share registration, callbacks, timeout, logging, and connection negotiation. Preserve existing errors, cancellation, and sensitive-URL redaction. Prove the behavior through an external consumer and existing manager regressions before advancing inventory evidence.
