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

pub use rdbc_array_access::RdbcArrayAccess;
pub use rdbc_blob_access::RdbcBlobAccess;
pub use rdbc_clob_access::RdbcClobAccess;
pub use rdbc_n_clob_access::RdbcNClobAccess;
pub use rdbc_ref_access::RdbcRefAccess;
pub use rdbc_resource_access::RdbcResourceAccess;
pub use rdbc_resource_capabilities::RdbcResourceCapabilities;
pub use rdbc_resource_context::RdbcResourceContext;
pub use rdbc_resource_factory::RdbcResourceFactory;
pub use rdbc_resource_id::RdbcResourceId;
pub use rdbc_resource_kind::RdbcResourceKind;
pub use rdbc_resource_owner::RdbcResourceOwner;
pub use rdbc_resource_state::RdbcResourceState;
pub use rdbc_sql_xml_access::RdbcSqlXmlAccess;
