//! MySQL Connector/J metadata compatibility policy。

/// Druid filter-owned MySQL DATETIME metadata compatibility flag。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MysqlMetadataCompatibility {
    mysql8_datetime: bool,
}

#[allow(dead_code)]
impl MysqlMetadataCompatibility {
    pub(crate) const fn mysql8_datetime() -> Self {
        Self {
            mysql8_datetime: true,
        }
    }

    pub(crate) fn class_name(self, class_name: &str) -> &str {
        if self.mysql8_datetime && class_name == "java.time.LocalDateTime" {
            "java.sql.Timestamp"
        } else {
            class_name
        }
    }
}
