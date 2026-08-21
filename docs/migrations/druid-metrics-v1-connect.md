# Metrics/Admin wire migration to `druid_metrics.v1::Connect`

## Removed contract

The pre-release Admin-owned wire contract was deleted rather than retained as a
second schema:

- package: `druid.admin.ingest`
- RPC: client-streaming `PushSnapshots`
- payload: `MetricsSnapshot`
- generated source/build owner: `druid-admin`

There is no wire compatibility between that contract and the current protocol.
Clients and Admin must be upgraded together; a mixed deployment fails closed.

## Current contract

`druid-metrics` is the only proto/build owner:

- package: `druid_metrics.v1`
- service: `MetricsIngest`
- only RPC: bidi `Connect(stream ClientFrame) returns (stream ServerFrame)`
- generated client/server features: `druid-metrics/client` and
  `druid-metrics/server`

The old snapshot maps to typed `SnapshotEntry` values inside `SnapshotBatch`.
Delivery now requires `ClientHello`/`HelloAck`, monotonic sequence and ACK,
immutable resend bytes, `ClientGoodbye`, and explicit resync. An authoritative
full snapshot additionally carries additive `SnapshotTransaction`
`transaction_id/chunk_index/chunk_count/end` metadata; Admin changes visible
state only after the complete transaction arrives. A one-chunk empty transaction
atomically clears that stream.

## Command and security changes

`ResetStats` and `RequestFullSnapshot` are server frames on the same bidi RPC.
Admin's protected `POST /druid/api/commands` uses the live stream identity and a
bounded `command_id + canonical payload + durable result` journal. State-changing
commands require an explicit Authorization header; a browser session alone is
not accepted. Same ID/same payload replays the stored result, while a payload
conflict fails closed.

Non-loopback clients must use an `https://` endpoint plus Authorization. The
client validates this before endpoint connect, RPC establishment, or metadata
injection; loopback h2c remains a local compatibility mode.

## Rollout

1. Deploy an Admin version that serves only `druid_metrics.v1::Connect`.
2. Deploy Metrics clients built from the same proto version and configure TLS +
   auth for non-loopback endpoints.
3. Verify Hello/full-snapshot convergence, command replay, `/health`, protected
   `/metrics`, and repository replacement counters.
4. Reject any artifact that still contains `PushSnapshots`, `MetricsSnapshot`,
   `druid.admin.ingest`, or a second metrics/ingest proto.
