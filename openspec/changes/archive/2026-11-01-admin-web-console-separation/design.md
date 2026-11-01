# Design: Admin web console with a separated front end

## Context

The standalone `druid-admin` is a push-based metrics receiver: nodes push `SnapshotEntry` batches over the gRPC ingest service, the admin keeps the latest value per datasource in a bounded in-memory repository, and the console reads that repository. The Java Druid admin works the other way round: it discovers the nodes and polls each node's `/druid/*.json`, which the node's `StatViewServlet` answers from the live pool.

That difference is why the console is unusable today. The push wire (`druid_metrics_v1.proto`) carries `PoolSnapshotMsg`, `SqlStatMsg` and three wall counters. Java's console pages read roughly 60 more datasource fields, the SQL parsed detail, the Wall table and function statistics, the pooled-connection rows and the active-connection stack traces. None of that fits the push shape, so the ported pages were rendered against an API that cannot answer them.

The Rust side already contains every piece needed to serve Java-equivalent data. `DruidDataSource` registers itself with `DruidDataSourceStatManager` on construction (`crates/druid/src/pool/druid_data_source.rs:314`), `DruidStatManagerFacade` aggregates the registered sources, `DruidStatService` renders the Java-compatible JSON for a servlet-style URL, and `StatViewServlet` mounts that rendering under `/druid`. The admin's `MonitorStatService` already discovers nodes and fans out to those URLs. What is missing is the wiring in the admin binary, the missing routes, and a front end that speaks a clean contract.

## Decisions

### Data plane: keep both backends, make the poll backend reachable

`MonitorBackend` already distinguishes `Remote(Arc<MonitorStatService>)` from `Repository(MetricsRepository)`. The admin binary only ever builds the repository one.

The admin will construct the remote backend as well and select it when the operator configures discovery. `MonitorStatService::new` needs a `DiscoveryClient`, a `MonitorProperties`, an optional `K8sDiscoveryClient` and an `HttpClient`; `StaticDiscoveryClient`, `KubeRsDiscoveryProvider` and `ReqwestHttpClient` already exist and `druid-admin` already depends on `reqwest`.

Both backends stay because they answer different questions. Push is the cheap, always-on, per-service view; it is the only source that works when a node cannot be reached over HTTP and the only source that survives the node's restart. Poll is the deep view: configuration, histograms, error detail, connection rows and stack traces exist only in the live process. The console composes them — push for liveness and totals, poll for the detail views.

The two backends are exposed through one backend-neutral accessor so no route handler has to know which one is active. Where a capability exists in only one of them, the API reports it as unsupported instead of returning zeros.

### API: a versioned contract plus the Java-compatible layer

The Java-shaped routes answer `{"ResultCode": 1, "Content": …}` for most payloads but a bare array for `serviceList.json`, a bare object for `/druid/sql/{id}` and `{"commandId","success"}` for commands. Field naming mixes Java PascalCase, camelCase for the Rust identity fields and lower camelCase for the parsed-SQL fields. A separated front end should not have to special-case each of those.

A new `/druid/api/v1` namespace carries one consistent contract: camelCase throughout, proper status codes instead of an error envelope — `401` for missing credentials, `404` for an unknown identity, `422` for a malformed one — and `application/json` on every response. The existing `.json` routes keep their current shape for embedders that already consume them, and gain the routes that are missing today.

The contract is published as an OpenAPI 3.1 document generated from the Rust types, so the front end's client and the documentation cannot drift from the implementation.

### Front end: Vite + Vue 3 + Element Plus, built into the binary

The front end lives in `crates/druid-admin/web/`. Vite 6 is already used by this repository's documentation site, so the toolchain is not new. Vue 3 with Element Plus is chosen because Element Plus's table already provides the sorting, pagination and fixed-column behaviour the Java pages implement by hand with `MapComparator` and `subList`.

`npm run build` emits `dist/`, and the admin binary embeds it with `rust-embed`. The admin stays a single self-contained executable, which preserves the property Java's console has of being embedded in the process that serves it, while the API the application talks to stays completely decoupled from the page delivery.

The hand-written `include_str!` match in `MonitorViewServlet::static_resource` is replaced by the embedded directory, which removes the need to edit Rust code to add an asset and gives content-hashed filenames for caching.

### Authentication: cookie for the served application, bearer for a separated deployment

The console is served from the admin itself, so the existing session cookie continues to work unchanged. A bearer credential is added alongside it so the application can be hosted on another origin. CORS is configurable and, when the admin is bound to a non-loopback address, the existing rule that TLS and authentication are both required is unchanged.

The redaction policy needs one adjustment. In secure mode it replaces `SQL`, `URL`, `UserName` and the error fields with `[REDACTED]`, which is right for a shared administrator but makes the SQL console useless for the operator who is already authenticated. Redaction stays the default for the compatibility routes; the versioned API serves an operator-scoped view whose redaction is driven by the role rather than by the presence of authentication.

### Page scope: data-bearing pages only

Java's console has sixteen pages. Four families — WebApp, WebURI, WebSession and Spring — are populated by servlet filters and by Spring's method interception. The Rust platform has neither, and the push protocol carries no such messages, so those pages can only ever be empty. The API reports them as unsupported with the reason, and the console shows that instead of a table with no rows.

The remaining pages are: index, datasource list and detail, SQL list and detail, Wall, pooled connections, active connection stack traces, and the API index.

## Risks

- **Polling cost.** A poll per node per request multiplies at the node count. The fan-out is already bounded by the discovered node set and each node answers locally, so the cost is one local HTTP call per node per request; the console's own polling interval is the control.
- **Two backends disagree.** Push and poll observe different moments. The console labels each figure with its source so a difference is explainable rather than mysterious.
- **Front-end build in the Rust build.** `npm` must be available for a full build. The embedding step is behind a build script that degrades to the previous pages when `dist/` is absent, so a Rust-only checkout still compiles.
- **Redaction relaxation.** Serving operator-scoped unredacted fields is a real exposure. It is scoped to the versioned API, requires the operator role, and is still redacted for the ingest role.

## Migration

The Java-compatible `.json` routes keep their current response shape. The ported doT pages are removed only after the separated application serves every page they served. Until then both are reachable, and the separated application is the default entry point.
