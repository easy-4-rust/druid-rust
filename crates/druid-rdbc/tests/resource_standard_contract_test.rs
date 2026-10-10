//! 独立RDBC资源合同；对应 Java: java.sql.Blob、Clob、NClob、Ref和Struct。
//! 独立保留六类资源及Struct的能力、访问与生命周期合同；正式Cargo验收另行记录。
#[path = "resource_standard_support/array_contract.rs"]
mod array_contract;
#[path = "resource_standard_support/blob_contract.rs"]
mod blob_contract;
#[path = "resource_standard_support/clob_contract.rs"]
mod clob_contract;
#[path = "resource_standard_support/context_contract.rs"]
mod context_contract;
#[path = "resource_standard_support/ref_contract.rs"]
mod ref_standard_contract;
#[path = "resource_standard_support/sql_xml_contract.rs"]
mod sql_xml_contract;
