//! Druid resource identity。

/// Druid owner registry 使用的稳定 resource id。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ResourceId(String);

#[allow(dead_code)]
impl ResourceId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn local() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
