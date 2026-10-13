//! Rust 专用的 standard extensions。

mod buffered_row_set;
mod character_reader;
mod character_writer;
mod connection_capabilities;
mod shared_result_set;
mod shared_result_set_borrow;
mod shared_result_set_borrow_future;
mod shared_result_set_state;
mod xml_result;
mod xml_source;

pub use crate::resource::RdbcArray as ArrayHandle;
pub use crate::resource::RdbcBlob as BlobHandle;
pub use crate::resource::RdbcCharacterLength as CharacterLength;
pub use crate::resource::RdbcClob as ClobHandle;
pub use crate::resource::RdbcInputStream as InputStreamHandle;
pub use crate::resource::RdbcNClob as NClobHandle;
pub use crate::resource::RdbcOutputStream as OutputStreamHandle;
pub use crate::resource::RdbcReader as ReaderHandle;
pub use crate::resource::RdbcRef as RefHandle;
pub use crate::resource::RdbcSqlXml as SqlXmlHandle;
pub use crate::resource::RdbcStreamLength as StreamLength;
pub use crate::resource::RdbcWriter as WriterHandle;
pub use crate::resource::RdbcXmlRepresentationType as XmlRepresentationType;
pub use crate::resource::RdbcXmlResult as XmlResultHandle;
pub use crate::resource::RdbcXmlSource as XmlSourceHandle;
pub use crate::statement::ExecResult;
pub use buffered_row_set::BufferedRowSet;
pub use character_reader::CharacterReader;
pub use character_writer::CharacterWriter;
pub use connection_capabilities::ConnectionCapabilities;
pub use shared_result_set::SharedResultSet;
pub use shared_result_set_borrow::SharedResultSetBorrow;
pub use xml_result::XmlResult;
pub use xml_source::XmlSource;

mod buffered_sql_input;
mod buffered_sql_output;
mod buffered_sql_struct;
mod sql_attribute;
pub use buffered_sql_input::BufferedSqlInput;
pub use buffered_sql_output::BufferedSqlOutput;
mod sql_data_object;
mod sql_data_registry;
pub use sql_data_object::SqlDataObject;
pub use sql_data_registry::SqlDataRegistry;
mod row_set_column_state;
mod row_set_meta_data;
mod row_set_meta_data_buffer;
pub use row_set_meta_data::RowSetMetaData;
pub use row_set_meta_data_buffer::RowSetMetaDataBuffer;
mod row_set_binding_key;
mod row_set_parameter_store;
pub use row_set_parameter_store::RowSetParameterStore;
mod row_set_properties;
pub use row_set_properties::RowSetProperties;
mod row_set_row_change;
mod row_set_row_store;
mod row_set_stored_row;
pub use row_set_row_change::RowSetRowChange;
pub use row_set_row_store::RowSetRowStore;

mod row_set;
pub use row_set::RowSet;

mod row_set_reader;
pub use row_set_reader::RowSetReader;

mod row_set_writer;
pub use row_set_writer::RowSetWriter;

mod row_set_listener;
pub use row_set_listener::RowSetListener;

mod row_set_event;
pub use row_set_event::RowSetEvent;

mod row_set_internal;
pub use row_set_internal::RowSetInternal;

mod shared_row_set_meta_data;
pub use shared_row_set_meta_data::SharedRowSetMetaData;

mod shared_row_set;
pub use shared_row_set::SharedRowSet;

mod row_set_parameter_resolver;
pub use row_set_parameter_resolver::RowSetParameterResolver;

mod row_set_cursor;

mod row_set_state;

mod row_set_execution_guard;

mod row_set_provider_context;
