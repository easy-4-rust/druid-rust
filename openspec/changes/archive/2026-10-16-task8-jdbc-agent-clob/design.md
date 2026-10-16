# Design

Use the existing Blob session ownership and owned executor patterns, but keep Clob identity and UTF-16 character semantics distinct from Blob bytes. The Agent stores each `java.sql.Clob` under an opaque session-local handle. Child Reader/Writer/InputStream/OutputStream handles retain the parent identity. A failed `Clob.free()` leaves the parent handle for retry; child-close failures aggregate with the driver failure, and logical/session close continues cleanup.

Protocol requests use bounded pages. Character values carry UTF-16 code units without replacing isolated surrogates; JDBC positions and lengths count UTF-16 units. The Rust bridge maps those units to `RdbcString` and its Reader/Writer contracts. Synchronous stream bridges must not block a Tokio runtime worker and must retain an explicit remote close result. Each capability bit requires a successful operation against a named real backend; unsupported overloads return `0A000` before driver access.

Implementation proceeds by TDD: session ownership, non-stream character operations, bounded streams, then external standard-consumer and pooled-owner checks. The frozen Java source oracle remains unchanged. The semantic inventory moves only after all Clob acceptance evidence exists.
