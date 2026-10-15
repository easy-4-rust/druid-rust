# Design

`DruidRdbcDataSource` keeps `Option<[u8; 32]>`. Construction hashes length-delimited username and password bytes with SHA-256 under a dedicated domain tag, then drops the owned input strings. A credential request hashes its borrowed inputs and compares the two 32-byte digests with `subtle::ConstantTimeEq`. Length framing prevents different `(username, password)` pairs from sharing the same byte stream. Missing configured credentials and mismatches take the existing authorization-error path, including safe diagnostics. `Debug` prints only whether a credential gate is configured.

The verifier is local to the wrapper; it does not alter public connection or source trait signatures. The digest is an in-process comparison token, not a persisted password hash. No raw credential bytes are logged or exposed through `Debug`.
