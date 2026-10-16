# Design

Use the existing `standard_backend_key_core()` so a raw adapter and its post-handoff actor report the same backend identity. Only `sqlite`, `mysql`, and `postgresql` produce positive transaction/savepoint flags; `any` and closed/unknown paths remain conservative. Extend the existing SQLx Any SQLite fixture with assertions on both sides of statement creation, and retain the native SQLite positive contract. Run targeted tests, formatter, Clippy, and the feature/CI matrix.
