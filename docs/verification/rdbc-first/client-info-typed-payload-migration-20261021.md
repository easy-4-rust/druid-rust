# ClientInfo typed payload migration — implementation in progress

This pre-release API change follows the explicit complete Rust/JDBC requirement and OpenSpec MC-11. Java SQLClientInfoException uses ClientInfoStatus values; arbitrary vendor strings are not members of that enum. Vendor error codes, SQLState, message and cause remain in the SQL diagnostics. This change is not a Java Druid behavior waiver.

The canonical SqlClientInfoException owns a nullable HashMap with nullable String keys and nullable ClientInfoStatus values. NULL map, empty map, NULL key, empty key and NULL status stay distinct. RdbcError::ClientInfoException now boxes this one canonical object; its former parallel string map/cause fields are removed. The raw DTO fields are encapsulated so a caller cannot replace the standard exception class with its cause's class. The nine Java constructor identities have corresponding Rust constructors. Inherited SQL getters are readable through Deref; set_next_exception has an explicit delegated method.

Replace struct literals and string failure maps with a constructor and enum entries:

```rust
use druid_rdbc::connection::ClientInfoStatus;
use druid_rdbc::error::{RdbcError, SqlClientInfoException};
use std::collections::HashMap;
let failure = SqlClientInfoException::with_details(
    Some("property rejected".to_owned()), Some("01S00".to_owned()), 7,
    Some(HashMap::from([(
        Some("ApplicationName".to_owned()),
        Some(ClientInfoStatus::ReasonValueInvalid),
    )])),
);
assert_eq!(failure.failed_properties().unwrap().len(), 1);
let error: RdbcError = failure.into();
assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
```

Use failed_properties()/failed_properties_mut(), cause(), and SQL getters instead of former fields. Match RdbcError::ClientInfoException { exception } when direct access is required. Existing already-typed exceptions retain their exact optional map through proxies; only the sequential default batch adds the names it knows were not attempted. Standard wrapping keeps the original error in the Rust source chain and preserves SQL diagnostics/next exceptions without marking the outer SQLClientInfoException as a SQLDataException.

Implementation, complete affected-module validation, current generated API/SemVer evidence and native adapter/real-database closure are still pending. No completion or production-readiness claim is made by this migration note.

References: [Java 8 SQLClientInfoException](https://docs.oracle.com/javase/8/docs/api/java/sql/SQLClientInfoException.html), [ClientInfoStatus](https://docs.oracle.com/javase/8/docs/api/java/sql/ClientInfoStatus.html).
