# P07 workspace and dependency gate

Checked on 2026-10-18, Windows/PowerShell, commit `0b0c3cb199211d92135b0885860e9463347a6879`. All commands below exited 0.

| Command | Observed result |
| --- | --- |
| `cargo metadata --no-deps --format-version 1 --offline --locked` | Exactly six workspace package IDs, each `path+file:///E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/<name>#0.0.0-design`: `druid`, `druid-admin`, `druid-bench`, `druid-metrics`, `druid-rdbc`, `druid-wrapper`. Each package reports `publish: []` (`publish = false`). |
| `python scripts/verify_workspace_dag.py` | Six packages, seven normal workspace edges, no cycle or forbidden edge. |
| `python scripts/verify_rdbc_dependency_boundary.py` | No forbidden reachable normal dependency from `druid-rdbc`. |
| `python scripts/verify_feature_dependency_boundaries.py` | Wrapper feature dependencies are optional and absent without default features. |
| `python scripts/verify_package_ownership.py` | Six workspace packages, two binaries, three driver and five live-contract CI invocations verified. |
| `cargo tree -p druid-rdbc -e normal --depth 1`, `cargo tree -p druid -e normal --depth 2`, `cargo tree -p druid-admin -e normal --depth 2` | Exit 0; direct `druid-rdbc` dependency list has no `druid`, `druid-wrapper`, `druid-metrics` or `druid-admin`; Admin reaches Metrics through one direct workspace edge. |

The seven normal workspace edges are: `druid → druid-rdbc`; `druid-admin → druid-metrics`; `druid-bench → druid`, `druid-wrapper`; `druid-metrics → druid`; and `druid-wrapper → druid`, `druid-rdbc`. Rejected edges: none. This proves the P07 Task 1 package/DAG gate only; feature powerset, runtime behavior and release readiness are separate gates.
