# Restore the wrapper import boundary gate

## Why

The boundary verifier only recognizes RDBC paths listed in the old-to-new migration map. Current public contracts with no former Druid path, starting with `driver::ConnectionFactory`, fail as unclassified, so the tracked import gate cannot inspect the live wrapper.

## What Changes

- Recognize current canonical declarations from the tracked `druid-rdbc` public API snapshot while retaining the frozen migration map for legacy paths.
- Keep unknown dependency symbols and wildcard imports fail-closed.
- Reconcile both boundary manifests with the current wrapper sources and report the exact inspected denominator.

## Capabilities

This is a verifier and evidence-manifest repair; `skip_specs: true` because runtime and public contract behavior do not change.

## Impact

Only the wrapper import verifier and its two tracked TSV manifests change. No Rust production code changes.
