# Proposal: SQLx fallback capability truthfulness

## Why

`SqlxConnectionAdapter` currently advertises transaction and savepoint support for every backend, including the generic `Any` fallback. The existing positive database contracts certify the native SQLite, MySQL, and PostgreSQL paths; they do not certify arbitrary fallback backends. A caller can therefore receive a capability claim that lacks evidence.

## What Changes

Derive the advertised transaction/savepoint flags from the actual backend identity before and after the connection moves into its owned actor. Keep the verified native backend claims, and report both flags as false for `Any` until a dedicated real-driver contract proves them. The operation implementations and SQL error propagation remain unchanged.
