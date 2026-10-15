//! 资源句柄与资源上下文合同的声明入口。

mod array;
mod blob;
mod clob;
mod n_clob;
mod rdbc_array;
mod rdbc_blob;
mod rdbc_clob;
mod rdbc_input_stream;
mod rdbc_n_clob;
mod rdbc_output_stream;
mod rdbc_reader;
mod rdbc_ref;
mod rdbc_resource_factory;
mod rdbc_resource_id;
mod rdbc_sql_xml;
mod rdbc_writer;
mod rdbc_xml_representation_type;
mod rdbc_xml_result;
mod rdbc_xml_source;
mod ref_value;
mod resource_operations;
mod sql_xml;
mod standard_resource;

pub use array::Array;
pub use blob::Blob;
pub use clob::Clob;
pub use n_clob::NClob;
pub use rdbc_array::RdbcArray;
pub use rdbc_blob::RdbcBlob;
pub use rdbc_clob::RdbcClob;
pub use rdbc_input_stream::RdbcInputStream;
pub use rdbc_input_stream::RdbcStreamLength;
pub use rdbc_n_clob::RdbcNClob;
pub use rdbc_output_stream::RdbcOutputStream;
pub use rdbc_reader::RdbcCharacterLength;
pub use rdbc_reader::RdbcReader;
pub use rdbc_ref::RdbcRef;
pub use rdbc_resource_factory::ResourceHandleFactory;
pub(crate) use rdbc_resource_id::ResourceIdentity;
pub use rdbc_sql_xml::RdbcSqlXml;
pub use rdbc_writer::RdbcWriter;
pub use rdbc_xml_representation_type::RdbcXmlRepresentationType;
pub use rdbc_xml_result::RdbcXmlResult;
pub use rdbc_xml_source::RdbcXmlSource;
pub use ref_value::Ref;
pub use resource_operations::ResourceOperations;
pub use sql_xml::SqlXml;
pub use standard_resource::Resource;

mod struct_value;
pub use struct_value::Struct;

mod rdbc_struct;
pub use rdbc_struct::RdbcStruct;

mod standard_resource_release_guard;
mod standard_resource_value;

mod blob_pattern;
pub use blob_pattern::BlobPattern;

mod clob_pattern;
pub use clob_pattern::ClobPattern;

mod sql_xml_value;

mod sql_data;
mod sql_input;
mod sql_output;
pub use sql_data::SqlData;
pub use sql_input::SqlInput;
pub use sql_output::SqlOutput;
