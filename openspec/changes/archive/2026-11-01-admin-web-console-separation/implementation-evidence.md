# Implementation evidence

## What was built

The separated console is delivered in two halves that meet at one contract.

**Backend.** The Admin gained the management routes its own bundled pages were
already calling and were not routed: `basic.json`, `datasource-{id}.json`,
`sql-{id}.json`, `connectionInfo-{id}.json`,
`activeConnectionStackTrace-{id}.json`, `reset-all.json` and
`log-and-reset.json`. Axum cannot express the identifier-in-the-middle form,
so those routes are resolved in the legacy dispatch after the static routes
miss. A capability the push protocol does not carry is reported as a named
reason instead of an empty success.

A versioned namespace `/druid/api/v1` carries the contract the separated
application reads: camelCase throughout, one status-code vocabulary
(`401` / `404` / `422` instead of an error envelope), and `application/json`
on every response. The Java-compatible `.json` routes keep their mixed
envelope and PascalCase fields for existing embedders.

**Front end.** `crates/druid-admin/web` is an independent Vite + Vue 3 +
Element Plus application. It talks only to `/druid/api/v1` and knows nothing
about how the Admin serves it. `crates/druid-admin/build.rs` reads the built
output and embeds it in the binary, so the Admin stays a single executable
while the API stays decoupled; adding an asset requires no Rust change. When
the application has not been built the script emits an empty table and the
Admin still compiles and still serves its API.

## Verification

### Module tests

| Suite | Result |
| --- | --- |
| `admin_console_routes_test` (new) | 9 passed, 0 failed |
| `admin_v1_api_test` (new) | 9 passed, 0 failed |
| `admin_console_app_test` (new) | 3 passed, 0 failed |

The three new suites cover the management routes, the versioned contract and
the console delivery. The Java-compatible routes and the versioned routes are
asserted against the same repository so the two cannot disagree.

The rest of the `druid-admin` suite is unchanged. The six
`grpc_network_e2e_test` failures are the timing flake already observed on
this host with an unmodified lock file (42/9, 38/13 and 5/46 on three
consecutive runs of the same unmodified tree); the same six names fail before
the change.

### Rendered acceptance

A real Admin process was started over a repository holding one datasource,
four SQL statistics and a Wall summary, and the console was driven in a
browser. The screenshots are in `gui-test-screenshots/`.

| View | Screenshot | Observation |
| --- | --- | --- |
| Datasources | `t1_datasources.png` | Header, runtime summary and the datasource table render; the row carries the identity, name, database type, driver, connection URL, pool counters and the Wall counters. |
| SQL statistics | `t2_sql_list.png` | Four statements render with the execution, error, timing and row counters; the sorted column carries the descending marker. |
| Wall | `t3_wall.png` | The eight Wall counters render, and the per-table, per-function and white-list tables show their empty state. |
| API index | `t4_api_index.png` | The seven versioned routes render from the published OpenAPI document. |

Read-only DOM checks confirmed the same values the API returns: the summary
row reads `0.0.0-design / 1 / 4 / postgresql / push`, the datasource row
reads `1 order-datasource postgresql postgres jdbc:postgresql://10.0.0.7:5432/orders 12 8 64 0 9482 7 4410 3 4 20480 3 order-service`, and the SQL view reports `SQL 统计（共 4 条）` with `执行次数 ▼` on the sorted column.

Navigation between the four views was performed through the rendered
navigation buttons.

### Environment preparation

The acceptance run used a throwaway example program that started the Admin
over a repository seeded with one datasource, four SQL statistics and a Wall
summary. It was removed after the run; the screenshots are the only
remaining artifact. No page behaviour was simulated: every value rendered
came from the live `/druid/api/v1` responses of a running Admin.

## Poll backend and authentication (Tasks 6-9 and 19-20)

The push ingest protocol carries about ten numeric fields, so the deep data
Java's console shows — pool configuration, histograms, error detail, pooled
connections and active-connection stack traces — exists only in the live
process. `ManagementSource` unifies the two data paths behind one policy,
remote first and repository fallback, and the servlet backend now holds that
source. Discovery is configured with `DRUID_ADMIN_DISCOVERY_MODE=push|poll|both`,
`DRUID_ADMIN_APPLICATIONS` and a comma-separated `DRUID_ADMIN_ENDPOINTS` list.
Kubernetes discovery is left as a follow-up.

Identity resolution keeps its composite key. The REST aliases address a
datasource by `(serviceId, identity)` and an SQL row by `(serviceId,
dataSourceId, id)`, so a service-name collision cannot return the wrong row;
the Java-compatible `/datasource-{id}.json` and `/sql-{id}.json` routes have
no `serviceId` and resolve by identity alone, matching Java.

Authentication for the separated console gained a bearer credential and
cross-origin support, and the versioned API now serves the operator view:
`DRUID_ADMIN_BEARER_TOKEN` configures a constant-time-checked token the
authorize middleware accepts alongside the session cookie and Basic
credentials; `DRUID_ADMIN_CORS_ORIGINS` lists the origins a separated
deployment is served from and an `AllowOrigin::list` layer answers their
preflight and refuses the rest. The versioned `/druid/api/v1` routes are no
longer redacted for an authenticated operator, while the Java-compatible
`.json` routes keep the conservative redaction.

### Additional module tests

| Suite | Result |
| --- | --- |
| `admin_poll_backend_test` (new) | 3 passed, 0 failed |
| `admin_bearer_cors_test` (new) | 4 passed, 0 failed |

The poll suite drives a fake monitored node over real HTTP and asserts the
Admin serves its `URL`/`UserName`, falls back to the repository when the node
is unreachable, and reports the configured application. The bearer suite
asserts a valid token grants operator access, a wrong or absent token
redirects, the versioned API keeps the operator view while the Java route
stays redacted, and CORS allows the configured origin while refusing others.

### Capability baseline

`docs/verification/rdbc-first/admin-console-capability.tsv` records, per Java
admin route, the capability, the Java response fields, the corresponding Rust
route, and its state (`LIVE`, `POLL_ONLY`, `PUSH_ONLY` or `UNSUPPORTED`). The
WebApp/WebURI/WebSession/Spring families are `UNSUPPORTED` because their data
comes from servlet filters and Spring interception, which the Rust platform
does not have; the API reports that rather than rendering empty tables.

The full druid-admin module suite passes; the intermittent
`grpc_network_e2e_test` failures remain the timing flake observed on this
host with an unmodified tree.
