# P04 Task 7: HTTP SQL and libSQL negative capability evidence

## Why

Task 7 still lacks executed capability evidence for enabled HTTP SQL and libSQL adapters. Both expose the canonical `Connection` interface, but their transaction, savepoint, callable, and SQL Array operations must not be inferred from interface presence.

## What changes

- Exercise both adapters through their standard connection factories and deterministic transports.
- Assert the five advertised connection capability flags are false and representative unsupported operations return their exact standard errors.
- Keep this evidence backend-scoped; a deterministic transport does not certify a live rqlite, Cloudflare D1, or remote libSQL service.

## Scope

This adds executable evidence only. It changes no public API or adapter behavior and does not close the remaining Task 7 backend matrix or Task 8 release gates.
