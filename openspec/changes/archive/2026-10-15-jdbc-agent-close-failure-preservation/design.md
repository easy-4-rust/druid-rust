# Design

Use the existing session `appendFailure` helper for the prepared-statement cascade. Apply the same first-failure/suppressed-failure pattern inside `AgentCursor.close()`. Both close paths already set their closed state or remove registry entries before invoking fallible JDBC closes, so error aggregation changes diagnostics without reintroducing resources.
