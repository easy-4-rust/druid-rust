# Design

The Java oracle remains unchanged. The verifier recognizes a narrow static declaration class: `DTO`, `ENUM_KIND`, or `MODULE`, with both adapter and logical implementation marked `NOT_APPLICABLE`; the active `DriverManager` registry is excluded. These rows may use `capability_state=NOT_APPLICABLE` and `evidence_level=V0_STATIC`. The verifier rejects that disposition for behavioral traits or rows with a live/planned adapter or logical implementation.

For source-hash comparison, only the approved static `NOT_APPLICABLE` capability value is normalized back to the frozen `PLANNED` value. The final gate still checks the on-disk classification and still fails on all unresolved behavior rows. This is an evidence classification, not a capability implementation or a blanket planned-state waiver.
