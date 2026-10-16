# Design

The verifier reads the existing migration map and the pinned public API snapshot. It accepts exact public declarations or descendants of those declarations, classifies the canonical module family, and preserves explicit product-only exceptions. A narrowly documented re-export path covers `error::RdbcError`, whose public API display path is the crate root. The snapshot must exist; unknown paths continue to fail. The generated manifests preserve the existing before/after source policy over the same live file-family keys.

Validation runs the verifier's negative controls and its full live-manifest check. This restores a gate that had stopped at the first unmapped canonical symbol.
