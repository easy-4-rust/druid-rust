# Design

## Context

The canonical `Blob` trait has eleven JDBC methods and synchronous stream handles; Druid already wraps returned resources with a lease registry. The Agent currently owns Arrays, cursors, and statements by session-local IDs. Its NDJSON response limit is 16 MiB, so a Blob cannot be sent as one unbounded value.

## Goals / Non-Goals

**Goals:** Keep driver Blob and stream identity behind opaque session-local handles; expose exact JDBC indexing and error behavior through the existing serialized `StandardExecutor`; make release and owner close deterministic.

**Non-Goals:** Change other adapters' Blob claims, introduce another pool, or certify databases beyond those actually tested.

## Decisions

1. Add a Blob registry to the Agent session and register child stream IDs against the parent. Reuse the Array ownership pattern. Close child streams before `Blob#free`, aggregate close errors, and retain the Blob ID when driver `free` fails. This avoids leaking resources across logical generations.
2. Use driver Blob methods directly for length, byte ranges, search, writes, and truncation. Protocol fields keep JDBC one-based positions distinct from zero-based input offsets. Byte payloads use Base64 with explicit per-request limits.
3. Stream reads/writes use bounded chunks and opaque child handles. A Rust synchronous stream bridge may block its caller while the existing serialized executor performs a bounded request; it must not buffer the complete Blob or hold an executor lock across caller code. End-of-stream and close are explicit protocol states.
4. Advertise each `ResourceOperations` bit only after every method represented by that bit has positive H2 evidence. Unsupported methods return `0A000`. The semantic inventory advances only after end-to-end behavior, close failure, and pooled-owner tests pass.

## Risks / Trade-offs

- A synchronous `Read`/`Write` facade over the async Agent can block its caller. Bound every transfer and test cancellation, shutdown, and owner close without deadlock.
- Driver behavior for `Blob#position(Blob,long)` and stream mutation differs across vendors. Preserve H2 behavior in this change and leave other backends uncertified until real-driver tests exist.
- A close or release error can leave remote state uncertain. Keep the original SQL error and make retry/owner cleanup behavior explicit in tests before promoting capabilities.
