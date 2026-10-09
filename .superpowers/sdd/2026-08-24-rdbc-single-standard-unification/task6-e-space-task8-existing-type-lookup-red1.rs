use druid_rdbc::types::RdbcType;
pub fn known_type_lookup() { let _ = RdbcType::value_of(4); }
pub fn unknown_type_lookup() { let _ = RdbcType::value_of(9999); }
