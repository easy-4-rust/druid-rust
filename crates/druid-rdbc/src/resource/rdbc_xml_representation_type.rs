//! Java XML Source/Result class token的 Rust 描述。

/// `SQLXML#getSource/setResult` 请求的 XML 表示类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RdbcXmlRepresentationType {
    /// `javax.xml.transform.stream.StreamSource/StreamResult`。
    Stream,
    /// `javax.xml.transform.sax.SAXSource/SAXResult`。
    Sax,
    /// `javax.xml.transform.stax.StAXSource/StAXResult`。
    Stax,
    /// `javax.xml.transform.dom.DOMSource/DOMResult`。
    Dom,
    /// 驱动或应用自定义实现类名。
    Custom(String),
}
