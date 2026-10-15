# P04 Task 7: cross-backend resource and ResultSet evidence

## Why

Task 7 has a focused live SQLx SQLite resource/ResultSet test, while the existing SQLx and Toasty SQLite/PostgreSQL/MySQL CI test checks only transactions and savepoints. The other adapter/backend paths therefore lack executed evidence for resource-factory rejection and asynchronous ResultSet lifecycle.

## What changes

- Extend the existing live capability contract to exercise all six unsupported resource factories through each standard connection.
- Exercise ordered rows, EOF, and explicit close through a standard Statement/ResultSet on the same live connection.
- Reuse the current SQLite and PostgreSQL/MySQL CI entries; record exactly which runs passed.

## Scope

This is an evidence and regression-test change. It does not implement resource factories, alter public APIs, or close all P04 Task 7/Task 8 acceptance gates.
