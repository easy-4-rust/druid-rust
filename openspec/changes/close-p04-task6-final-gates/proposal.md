# Close P04 Task 6 with current evidence

## Why

The physical-to-standard migration is recorded, but Task 6 is not closed until its source-purity, behavior-preservation, public API, documentation, feature, and toolchain checks are reproducible against the current checkout. Historical evidence alone cannot certify the current tree.

## What changes

- Reconcile the migration authority, physical-standardization manifest, and generated migration document.
- Re-run exact method and deleted-test parity checks, refreshing only evidence whose source identity has changed.
- Generate and compare the current `druid-rdbc` public API snapshot, then verify rustdoc and SemVer compatibility.
- Run the required workspace, feature, and MSRV checks and record exact commands, outcomes, and any environment-blocked gates.

## Scope and non-goals

This change closes P04 Task 6 only. It does not claim P04 Tasks 7–8 or P05–P07 are complete, and it does not introduce new public runtime behavior unless a failed Task 6 contract check proves a migration defect. Any such defect must be specified and tested before implementation.

## Acceptance

- Every retired public identity has exactly one valid disposition and target; authority and generated documentation agree.
- All preserved method and test identities have current, source-bound evidence; no behavioral test is silently dropped.
- The public API matches the reviewed snapshot, rustdoc emits no warnings, and SemVer results are recorded.
- Required workspace, feature, and MSRV commands have actual terminal results. Environment-blocked checks remain explicitly open and prevent closure.
- OpenSpec validation passes, the change is synced only after acceptance, and the accepted feature point is committed and pushed.
