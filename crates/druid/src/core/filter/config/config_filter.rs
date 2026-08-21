#![allow(clippy::case_sensitive_file_extension_comparisons)]
//! 对应 Java：`com.alibaba.druid.filter.config.ConfigFilter`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/config/ConfigFilter.java`。

use crate::core::filter_manager::parse_java_properties;
use crate::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, ResultSetFilter,
};
use quick_xml::de::from_reader;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[allow(deprecated)]
use super::ConfigTools;

const CONFIG_FILTER_CLASS: &str = "com.alibaba.druid.filter.config.ConfigFilter";

/// 数据源配置下载与旧 Druid RSA 密文解密 Filter。
///
/// Java 对象在 `DruidDataSource#init` 时读取连接属性，按连接属性、远程配置、
/// JVM system property 的既定优先级完成初始化。Rust 没有 JVM system property
/// 和 classpath，因而分别映射为可注入属性表与显式资源根；默认入口仅为三个
/// `druid.config.*` 键读取同名进程环境变量。HTTP(S) 使用 rustls-backed
/// `reqwest`，不会引入第二套数据库或连接池。
#[derive(Debug, Clone)]
pub struct ConfigFilter {
    #[cfg(feature = "config-http")]
    http_client: reqwest::Client,
    classpath_roots: Vec<PathBuf>,
}

impl ConfigFilter {
    /// 远程或本地配置文件位置。
    pub const CONFIG_FILE: &'static str = "config.file";
    /// 是否解密密码。
    pub const CONFIG_DECRYPT: &'static str = "config.decrypt";
    /// Base64 X.509/SPKI RSA 公钥。
    pub const CONFIG_KEY: &'static str = "config.decrypt.key";

    /// Java system property 对应的配置文件键。
    pub const SYS_PROP_CONFIG_FILE: &'static str = "druid.config.file";
    /// Java system property 对应的解密开关键。
    pub const SYS_PROP_CONFIG_DECRYPT: &'static str = "druid.config.decrypt";
    /// Java system property 对应的公钥键。
    pub const SYS_PROP_CONFIG_KEY: &'static str = "druid.config.decrypt.key";

    /// `DruidDataSourceFactory` 的连接属性字符串键。
    pub const CONNECTION_PROPERTIES: &'static str = "connectionProperties";
    /// `DruidDataSourceFactory` 的密码键。
    pub const PASSWORD: &'static str = "password";

    /// 创建默认配置 Filter。
    ///
    /// 相对路径先按真实文件解析；文件不存在时，再从当前工作目录作为 Rust
    /// classpath 根查找。
    #[must_use]
    pub fn new() -> Self {
        let classpath_roots = std::env::current_dir().into_iter().collect();
        Self {
            #[cfg(feature = "config-http")]
            http_client: reqwest::Client::new(),
            classpath_roots,
        }
    }

    /// 使用调用方提供的资源根创建 Filter。
    ///
    /// 该入口用于宿主注入确定性的 classpath 替代目录。
    #[must_use]
    pub fn with_runtime(classpath_roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            #[cfg(feature = "config-http")]
            http_client: reqwest::Client::new(),
            classpath_roots: classpath_roots.into_iter().collect(),
        }
    }

    /// 使用调用方提供的 HTTP client 和资源根创建 Filter。
    ///
    /// 该入口用于宿主注入超时、代理、证书和确定性的 classpath 替代目录。
    #[cfg(feature = "config-http")]
    #[must_use]
    pub fn with_http_client(
        http_client: reqwest::Client,
        classpath_roots: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        Self {
            http_client,
            classpath_roots: classpath_roots.into_iter().collect(),
        }
    }

    /// 判断工厂属性是否启用了 Java `config` Filter。
    #[must_use]
    pub fn is_enabled(data_source_properties: &HashMap<String, String>) -> bool {
        data_source_properties
            .get("filters")
            .map(String::as_str)
            .map(|filters| filters.strip_prefix('!').unwrap_or(filters))
            .is_some_and(|filters| {
                filters.split(',').any(|filter| {
                    let filter = trim_rdbc_string(filter);
                    filter == "config" || filter == CONFIG_FILTER_CLASS
                })
            })
    }

    /// 使用同名进程环境变量替代 JVM system properties，解析完整数据源属性。
    ///
    /// 返回值是 Java `DruidDataSourceFactory.config(dataSource, info)` 执行后的
    /// 等价属性视图：远程配置覆盖原属性，未覆盖项保持不变。
    ///
    /// # Errors
    ///
    /// 配置文件不可读、格式非法、HTTP 状态失败或密码解密失败时返回带 Java
    /// 原始消息前缀的错误。
    pub async fn resolve_properties(
        &self,
        data_source_properties: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, DruidError> {
        let system_properties = [
            Self::SYS_PROP_CONFIG_FILE,
            Self::SYS_PROP_CONFIG_DECRYPT,
            Self::SYS_PROP_CONFIG_KEY,
        ]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
        .collect();
        self.resolve_properties_with_system(data_source_properties, &system_properties)
            .await
    }

    /// 使用显式 system property 表解析完整数据源属性。
    ///
    /// 该入口保持 Java 三段优先级，同时让测试与非 JVM 宿主无需修改全局环境。
    pub async fn resolve_properties_with_system(
        &self,
        data_source_properties: &HashMap<String, String>,
        system_properties: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, DruidError> {
        let connection_properties = data_source_properties
            .get(Self::CONNECTION_PROPERTIES)
            .map(|source| parse_connection_properties(source))
            .unwrap_or_default();

        let mut config_file_properties = self
            .load_property_from_config_file(&connection_properties, system_properties)
            .await?;
        let decrypt = self.is_decrypt(
            &connection_properties,
            config_file_properties.as_ref(),
            system_properties,
        );

        if decrypt {
            self.decrypt_properties(
                data_source_properties,
                &connection_properties,
                config_file_properties.as_mut(),
                system_properties,
            )?;
        }

        let mut resolved = data_source_properties.clone();
        if let Some(config_file_properties) = config_file_properties {
            resolved.extend(config_file_properties);
        } else if decrypt {
            let encrypted_password = connection_properties
                .get(Self::PASSWORD)
                .filter(|value| !value.is_empty())
                .or_else(|| data_source_properties.get(Self::PASSWORD));
            let public_key = self.public_key_text(&connection_properties, None, system_properties);
            #[allow(deprecated)]
            let decrypted = ConfigTools::decrypt_with_public_key_text(
                public_key,
                encrypted_password.map(String::as_str),
            )
            .map_err(|error| DruidError::Other(format!("Failed to decrypt. {error}")))?;
            match decrypted {
                Some(password) => {
                    resolved.insert(Self::PASSWORD.to_owned(), password);
                }
                None => {
                    resolved.remove(Self::PASSWORD);
                }
            }
        }
        Ok(resolved)
    }

    /// 按 Java `Boolean.valueOf` 规则判断是否需要解密。
    #[must_use]
    pub fn is_decrypt(
        &self,
        connection_properties: &HashMap<String, String>,
        config_file_properties: Option<&HashMap<String, String>>,
        system_properties: &HashMap<String, String>,
    ) -> bool {
        first_non_empty([
            connection_properties.get(Self::CONFIG_DECRYPT),
            config_file_properties.and_then(|info| info.get(Self::CONFIG_DECRYPT)),
            system_properties.get(Self::SYS_PROP_CONFIG_DECRYPT),
        ])
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    }

    /// 加载 Java properties 或 XML properties 配置。
    ///
    /// `file://`、HTTP(S)、`classpath:` 和普通路径的分支顺序与 Java 一致。
    /// 普通路径不存在时会继续在显式 classpath 根中查找。
    ///
    /// # Errors
    ///
    /// 读取、HTTP 或 properties 解析失败时返回底层结构化错误。
    pub async fn load_config(
        &self,
        file_path: &str,
    ) -> Result<HashMap<String, String>, DruidError> {
        let (bytes, xml) = if let Some(path) = file_path.strip_prefix("file://") {
            (
                read_file_or_classpath(Path::new(path), &self.classpath_roots).await?,
                path.ends_with(".xml"),
            )
        } else if file_path.starts_with("http://") || file_path.starts_with("https://") {
            #[cfg(not(feature = "config-http"))]
            {
                return Err(DruidError::UnsupportedOperation {
                    operation: "config_http_requires_config_http_feature",
                });
            }
            #[cfg(feature = "config-http")]
            {
                let xml = url::Url::parse(file_path)
                    .map_err(|error| DruidError::Other(error.to_string()))?
                    .path()
                    .ends_with(".xml");
                let response = self
                    .http_client
                    .get(file_path)
                    .send()
                    .await
                    .and_then(reqwest::Response::error_for_status)
                    .map_err(|error| DruidError::Other(error.to_string()))?;
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|error| DruidError::Other(error.to_string()))?
                    .to_vec();
                (bytes, xml)
            }
        } else if let Some(resource_path) = file_path.strip_prefix("classpath:") {
            (
                read_classpath(Path::new(resource_path), &self.classpath_roots).await?,
                resource_path.ends_with(".xml"),
            )
        } else {
            (
                read_file_or_classpath(Path::new(file_path), &self.classpath_roots).await?,
                file_path.ends_with(".xml"),
            )
        };

        if xml {
            parse_xml_properties(&bytes)
        } else {
            // Java Properties.load(InputStream) 固定按 ISO-8859-1 将每个字节映射
            // 为同码位字符，非 ASCII 内容应通过 \uXXXX 表达。
            let source: String = bytes.into_iter().map(char::from).collect();
            parse_java_properties(&source)
        }
    }

    async fn load_property_from_config_file(
        &self,
        connection_properties: &HashMap<String, String>,
        system_properties: &HashMap<String, String>,
    ) -> Result<Option<HashMap<String, String>>, DruidError> {
        let Some(config_file) = connection_properties
            .get(Self::CONFIG_FILE)
            .or_else(|| system_properties.get(Self::SYS_PROP_CONFIG_FILE))
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };

        tracing::info!(config_file, "DruidDataSource Config File load from");
        self.load_config(config_file)
            .await
            .map(Some)
            .map_err(|error| {
                tracing::error!(config_file, %error, "load config file error");
                DruidError::Other(format!(
                    "Cannot load remote config file from the [config.file={config_file}]."
                ))
            })
    }

    fn decrypt_properties(
        &self,
        data_source_properties: &HashMap<String, String>,
        connection_properties: &HashMap<String, String>,
        config_file_properties: Option<&mut HashMap<String, String>>,
        system_properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        let encrypted_password = first_non_empty([
            config_file_properties
                .as_deref()
                .and_then(|info| info.get(Self::PASSWORD)),
            connection_properties.get(Self::PASSWORD),
            data_source_properties.get(Self::PASSWORD),
        ]);
        let public_key = self.public_key_text(
            connection_properties,
            config_file_properties.as_deref(),
            system_properties,
        );
        #[allow(deprecated)]
        let decrypted = ConfigTools::decrypt_with_public_key_text(
            public_key,
            encrypted_password.map(String::as_str),
        )
        .map_err(|error| DruidError::Other(format!("Failed to decrypt. {error}")))?;

        if let Some(info) = config_file_properties {
            let password = decrypted.ok_or_else(|| {
                DruidError::Other(
                    "Failed to decrypt. encrypted password must not be null".to_owned(),
                )
            })?;
            info.insert(Self::PASSWORD.to_owned(), password);
        }
        Ok(())
    }

    fn public_key_text<'a>(
        &self,
        connection_properties: &'a HashMap<String, String>,
        config_file_properties: Option<&'a HashMap<String, String>>,
        system_properties: &'a HashMap<String, String>,
    ) -> Option<&'a str> {
        first_non_empty([
            config_file_properties.and_then(|info| info.get(Self::CONFIG_KEY)),
            connection_properties.get(Self::CONFIG_KEY),
            system_properties.get(Self::SYS_PROP_CONFIG_KEY),
        ])
        .map(String::as_str)
    }
}

impl Default for ConfigFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for ConfigFilter {
    fn name(&self) -> &str {
        "config"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for ConfigFilter {
    fn name(&self) -> &str {
        "config"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

impl ResultSetFilter for ConfigFilter {}

#[derive(Debug, Deserialize)]
#[serde(rename = "properties")]
struct XmlProperties {
    #[serde(rename = "entry", default)]
    entries: Vec<XmlPropertyEntry>,
}

#[derive(Debug, Deserialize)]
struct XmlPropertyEntry {
    #[serde(rename = "@key")]
    key: String,
    #[serde(rename = "$text", default)]
    value: String,
}

fn parse_xml_properties(bytes: &[u8]) -> Result<HashMap<String, String>, DruidError> {
    let document: XmlProperties = from_reader(bytes)
        .map_err(|error| DruidError::InvalidArgument(format!("invalid XML properties: {error}")))?;
    Ok(document
        .entries
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect())
}

fn parse_connection_properties(source: &str) -> HashMap<String, String> {
    source
        .split(';')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry.find('=').filter(|index| *index > 0).map_or_else(
                || (entry.to_owned(), String::new()),
                |index| (entry[..index].to_owned(), entry[index + 1..].to_owned()),
            )
        })
        .collect()
}

fn first_non_empty<const N: usize>(values: [Option<&String>; N]) -> Option<&String> {
    values.into_iter().flatten().find(|value| !value.is_empty())
}

async fn read_file_or_classpath(
    path: &Path,
    classpath_roots: &[PathBuf],
) -> Result<Vec<u8>, DruidError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            read_classpath(path, classpath_roots).await
        }
        Err(error) => Err(DruidError::Other(error.to_string())),
    }
}

async fn read_classpath(
    resource_path: &Path,
    classpath_roots: &[PathBuf],
) -> Result<Vec<u8>, DruidError> {
    for root in classpath_roots {
        let candidate = root.join(resource_path);
        match tokio::fs::read(&candidate).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(DruidError::Other(error.to_string())),
        }
    }
    Err(DruidError::Other(format!(
        "config resource not found: {}",
        resource_path.display()
    )))
}

fn trim_rdbc_string(value: &str) -> &str {
    value.trim_matches(|character| character <= '\u{20}')
}
