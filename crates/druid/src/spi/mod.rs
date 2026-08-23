//! Driver extension points for RDBC resources.
//!
//! Applications use the concrete handles in [`crate::rdbc`]. Database adapters implement the
//! access traits in this module and create handles through [`crate::spi::RdbcResourceFactory`].

pub mod driver_extension_descriptor;
pub mod driver_extension_registry;
mod rdbc_array_access;
mod rdbc_blob_access;
mod rdbc_clob_access;
mod rdbc_n_clob_access;
mod rdbc_ref_access;
mod rdbc_resource_access;
mod rdbc_resource_capabilities;
mod rdbc_resource_context;
mod rdbc_resource_factory;
mod rdbc_resource_id;
mod rdbc_resource_kind;
mod rdbc_resource_owner;
mod rdbc_resource_state;
mod rdbc_sql_xml_access;

pub use druid_rdbc::resource::RdbcArrayAccess;
pub use druid_rdbc::resource::RdbcBlobAccess;
pub use druid_rdbc::resource::RdbcClobAccess;
pub use druid_rdbc::resource::RdbcNClobAccess;
pub use druid_rdbc::resource::RdbcRefAccess;
pub use druid_rdbc::resource::RdbcResourceAccess;
pub use druid_rdbc::resource::RdbcResourceCapabilities;
pub use druid_rdbc::resource::RdbcResourceContext;
pub use druid_rdbc::resource::RdbcResourceFactory;
pub use druid_rdbc::resource::RdbcResourceId;
pub use druid_rdbc::resource::RdbcResourceKind;
pub use druid_rdbc::resource::RdbcResourceOwner;
pub use druid_rdbc::resource::RdbcResourceState;
pub use druid_rdbc::resource::RdbcSqlXmlAccess;
