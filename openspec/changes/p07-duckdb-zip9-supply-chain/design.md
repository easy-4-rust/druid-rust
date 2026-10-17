# Design

Use the smallest compatible upstream DuckDB binding update rather than maintaining a local ZIP fork. `libduckdb-sys 1.10506.1` declares `zip 9`; the current lockfile contains `libduckdb-sys 1.10505.0` and `zip 6.0.0`. Update the workspace pin and resolve the lockfile, then verify the actual dependency tree and build on supported targets.

The CI audit must pass without adding the three ZIP advisories to an ignore list. If the upstream update changes adapter behavior, keep the change open and fix the adapter before claiming completion.
