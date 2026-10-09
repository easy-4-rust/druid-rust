//! 资源句柄与资源上下文合同的声明入口。

mod array;
mod rdbc_array;
mod blob;
mod rdbc_blob;
mod clob;
mod rdbc_clob;
mod n_clob;
mod rdbc_n_clob;
mod rdbc_clob_access;
mod rdbc_input_stream;
mod rdbc_n_clob_access;
mod rdbc_output_stream;
mod rdbc_reader;
mod rdbc_resource_access;
mod rdbc_resource_capabilities;
mod rdbc_resource_context;
mod rdbc_resource_factory;
mod rdbc_resource_id;
mod rdbc_resource_kind;
mod rdbc_resource_state;
mod rdbc_sql_xml_access;
mod rdbc_writer;
mod rdbc_xml_representation_type;
mod rdbc_xml_result;
mod rdbc_xml_source;
mod ref_value;
mod rdbc_ref;
mod sql_xml;
mod rdbc_sql_xml;

pub use array::Array;
pub use rdbc_array::RdbcArray;
pub use blob::Blob;
pub use rdbc_blob::RdbcBlob;
pub use clob::Clob;
pub use rdbc_clob::RdbcClob;
pub use n_clob::NClob;
pub use rdbc_n_clob::RdbcNClob;
pub use rdbc_clob_access::RdbcClobAccess;
pub use rdbc_input_stream::RdbcInputStream;
pub use rdbc_input_stream::RdbcStreamLength;
pub use rdbc_n_clob_access::RdbcNClobAccess;
pub use rdbc_output_stream::RdbcOutputStream;
pub use rdbc_reader::RdbcCharacterLength;
pub use rdbc_reader::RdbcReader;
pub use rdbc_resource_access::RdbcResourceAccess;
pub use rdbc_resource_capabilities::ResourceOperations;
pub(crate) use rdbc_resource_context::ResourceLifecycle;
pub use rdbc_resource_factory::ResourceHandleFactory;
pub(crate) use rdbc_resource_id::ResourceIdentity;
pub(crate) use rdbc_resource_kind::ResourceType;
pub(crate) use rdbc_resource_state::ResourceLifecycleState;
pub use rdbc_sql_xml_access::RdbcSqlXmlAccess;
pub use rdbc_writer::RdbcWriter;
pub use rdbc_xml_representation_type::RdbcXmlRepresentationType;
pub use rdbc_xml_result::RdbcXmlResult;
pub use rdbc_xml_source::RdbcXmlSource;
pub use rdbc_ref::RdbcRef;
pub use ref_value::Ref;
pub use rdbc_sql_xml::RdbcSqlXml;
pub use sql_xml::SqlXml;

mod struct_value;
pub use struct_value::Struct;

mod rdbc_struct;
pub use rdbc_struct::RdbcStruct;

mod standard_resource_value;
mod standard_resource_release_guard;

mod blob_pattern;
pub use blob_pattern::BlobPattern;
