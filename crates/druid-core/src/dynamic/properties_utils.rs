//! 对应 Java 类：`com.alibaba.druid.pool.ha.PropertiesUtils`。

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Java properties 的 HA 节点配置工具。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.PropertiesUtils`。解析委托给遵循
/// `java.util.Properties` 转义规则的 `java-properties`，不会用临时的
/// `split('=')` 破坏续行、Unicode 转义和转义分隔符语义。
pub struct PropertiesUtils;

impl PropertiesUtils {
    /// 从文件系统读取 properties；读取或解析失败时与 Java 版一致返回空集合。
    #[must_use]
    pub fn load_properties(file: Option<&Path>) -> HashMap<String, String> {
        let Some(file) = file else {
            return HashMap::new();
        };
        let result = File::open(file)
            .map(BufReader::new)
            .map_err(|error| error.to_string())
            .and_then(|reader| java_properties::read(reader).map_err(|error| error.to_string()));
        match result {
            Ok(properties) => properties,
            Err(error) => {
                tracing::warn!(file = %file.display(), error = %error, "无法加载 HA 数据源配置");
                HashMap::new()
            }
        }
    }

    /// 提取以 `.url` 结尾且符合前缀过滤条件的节点名。
    #[must_use]
    pub fn load_name_list(
        properties: &HashMap<String, String>,
        property_prefix: Option<&str>,
    ) -> Vec<String> {
        let prefix = property_prefix.unwrap_or_default();
        let names: HashSet<String> = properties
            .keys()
            .filter(|name| prefix.is_empty() || name.starts_with(prefix))
            .filter_map(|name| name.strip_suffix(".url").map(ToOwned::to_owned))
            .collect();
        names.into_iter().collect()
    }

    /// 仅保留指定前缀的属性；空前缀返回内容相同的集合。
    #[must_use]
    pub fn filter_prefix(
        properties: &HashMap<String, String>,
        prefix: Option<&str>,
    ) -> HashMap<String, String> {
        let prefix = prefix.unwrap_or_default();
        if prefix.is_empty() {
            return properties.clone();
        }
        properties
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }
}
