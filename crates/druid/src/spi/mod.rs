//! Driver extension points for RDBC resources.
//!
//! Applications use the concrete handles in [`crate::sql`]. Database adapters implement the
//! access traits in this module and create handles through [`crate::spi::ResourceHandleFactory`].

pub mod driver_extension_descriptor;
pub mod driver_extension_registry;
mod rdbc_array_access;
mod rdbc_blob_access;
mod rdbc_clob_access;
mod rdbc_n_clob_access;
mod rdbc_ref_access;
mod rdbc_resource_access;
mod rdbc_resource_capabilities;
mod rdbc_sql_xml_access;

pub use druid_rdbc::resource::RdbcArrayAccess;
pub use druid_rdbc::resource::RdbcBlobAccess;
pub use druid_rdbc::resource::RdbcClobAccess;
pub use druid_rdbc::resource::RdbcNClobAccess;
pub use druid_rdbc::resource::RdbcRefAccess;
pub use druid_rdbc::resource::RdbcResourceAccess;
pub use druid_rdbc::resource::RdbcSqlXmlAccess;
pub use druid_rdbc::resource::ResourceHandleFactory;
pub use druid_rdbc::resource::ResourceOperations;
