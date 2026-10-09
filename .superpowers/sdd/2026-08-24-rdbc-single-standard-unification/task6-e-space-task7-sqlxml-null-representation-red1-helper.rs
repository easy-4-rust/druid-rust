use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcSqlXml,RdbcXmlSource,RdbcXmlResult};
pub async fn vendor_default_source(xml: &RdbcSqlXml) -> Result<RdbcXmlSource,RdbcError> { xml.source(None).await }
pub async fn vendor_default_result(xml: &RdbcSqlXml) -> Result<RdbcXmlResult,RdbcError> { xml.result(None).await }
fn main() {}
