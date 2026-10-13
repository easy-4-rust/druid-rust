//! Task 7 迁移权威表的公开资源路径编译合同。

use druid_rdbc::extensions::{
    ArrayHandle, BlobHandle, CharacterLength, ClobHandle, InputStreamHandle, NClobHandle,
    OutputStreamHandle, ReaderHandle, RefHandle, SqlXmlHandle, StreamLength, WriterHandle,
    XmlRepresentationType, XmlResultHandle, XmlSourceHandle,
};
use druid_rdbc::resource::{
    RdbcArray, RdbcBlob, RdbcCharacterLength, RdbcClob, RdbcInputStream, RdbcNClob,
    RdbcOutputStream, RdbcReader, RdbcRef, RdbcSqlXml, RdbcStreamLength, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
};

#[test]
fn task7_extension_paths_preserve_the_existing_resource_identity() {
    // 迁移目标必须可由外部 crate 导入，并保持当前标准资源句柄的类型身份。
    let _: Option<ArrayHandle> = None::<RdbcArray>;
    let _: Option<BlobHandle> = None::<RdbcBlob>;
    let _: Option<CharacterLength> = None::<RdbcCharacterLength>;
    let _: Option<ClobHandle> = None::<RdbcClob>;
    let _: Option<InputStreamHandle> = None::<RdbcInputStream>;
    let _: Option<NClobHandle> = None::<RdbcNClob>;
    let _: Option<OutputStreamHandle> = None::<RdbcOutputStream>;
    let _: Option<ReaderHandle> = None::<RdbcReader>;
    let _: Option<RefHandle> = None::<RdbcRef>;
    let _: Option<SqlXmlHandle> = None::<RdbcSqlXml>;
    let _: Option<StreamLength> = None::<RdbcStreamLength>;
    let _: Option<WriterHandle> = None::<RdbcWriter>;
    let _: Option<XmlRepresentationType> = None::<RdbcXmlRepresentationType>;
    let _: Option<XmlResultHandle> = None::<RdbcXmlResult>;
    let _: Option<XmlSourceHandle> = None::<RdbcXmlSource>;
}
