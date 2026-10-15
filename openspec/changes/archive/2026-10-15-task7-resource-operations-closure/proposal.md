# Close the ResourceOperations consumer migration

## Why

The 108 frozen Task 5 resource-capability consumers still report Task 7 pending even though production resource traits use the standard `ResourceOperations` type. The old manifest's proposed Druid-private target is historical and cannot certify the actual standard target.

## What Changes

Keep the frozen consumer inventory intact. Add an exact 108-row closure ledger mapping each historical occurrence to `druid_rdbc::resource::ResourceOperations`, require the old symbol to be absent from production source, and run the standard resource contract before treating these consumer rows as closed. Move the standard type to a matching Rust source file. Other Task 7 resource identities and final semantic inventory remain open.
