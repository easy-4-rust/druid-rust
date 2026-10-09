#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-array-candidate1.rs"]
mod canonical_array;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-clob-candidate1.rs"]
mod canonical_clob;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-reference-candidate1.rs"]
mod canonical_reference;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-sql_xml-candidate1.rs"]
mod canonical_sql_xml;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-n_clob-candidate1.rs"]
mod canonical_n_clob;
pub type ArrayObject = Box<dyn canonical_array::Array>;
pub type ClobObject = Box<dyn canonical_clob::Clob>;
pub type RefObject = Box<dyn canonical_reference::Ref>;
pub type SqlXmlObject = Box<dyn canonical_sql_xml::SqlXml>;
pub type NClobObject = Box<dyn canonical_n_clob::NClob>;
pub fn n_clob_is_clob(value: &mut dyn canonical_n_clob::NClob) -> &mut dyn canonical_clob::Clob { value }
fn main() {}

pub fn vendor_default_source_send(value: &mut dyn canonical_sql_xml::SqlXml) -> impl std::future::Future<Output = Result<druid_rdbc::resource::RdbcXmlSource, druid_rdbc::error::RdbcError>> + Send + '_ { value.source(None) }
pub fn vendor_default_result_send(value: &mut dyn canonical_sql_xml::SqlXml) -> impl std::future::Future<Output = Result<druid_rdbc::resource::RdbcXmlResult, druid_rdbc::error::RdbcError>> + Send + '_ { value.result(None) }
