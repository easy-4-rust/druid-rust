# Preserve JDBC Agent resource close failures

## Why

When a prepared statement owns multiple cursors, closing the statement currently replaces earlier cursor close failures with the final failure. A cursor also replaces a ResultSet close failure when its temporary statement fails to close. This loses the first diagnostic and hides subsequent failures.

## What changes

- Preserve the first `SQLException` from a close cascade and attach later close failures as suppressed exceptions.
- Continue attempting to close every related resource and remove session registrations even when a close fails.
- Add deterministic tests for both cursor-local and prepared-statement cascades.

## Scope

JDBC Agent internal resource lifecycle only. No wire protocol or public Rust API changes. This does not close the broader P07 reliability or release gates.
