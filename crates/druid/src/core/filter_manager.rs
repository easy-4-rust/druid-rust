//! 对应 Java：`com.alibaba.druid.filter.FilterManager`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/FilterManager.java`。

use super::{AfterFilter, BeforeFilter, DruidError, FilterChain, ResultSetFilter};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

const FILTER_PREFIX: &str = "druid.filters.";
const BUNDLED_FILTER_PROPERTIES: &str =
    include_str!("../../resources/META-INF/druid-filter.properties");

struct RegisteredFilter {
    class_name: String,
    before: Arc<dyn BeforeFilter>,
    after: Arc<dyn AfterFilter>,
    result_set: Arc<dyn ResultSetFilter>,
}

type FilterFactory = dyn Fn() -> Result<RegisteredFilter, DruidError> + Send + Sync + 'static;

/// Druid Filter 别名与构造工厂管理器。
///
/// Java 通过三个 `ClassLoader` 合并 `META-INF/druid-filter.properties`，再用
/// `Class#newInstance()` 构造 Filter。Rust 没有 classpath，因而保留同一别名、
/// 覆盖、逗号展开、去重及错误语义，同时用显式注册工厂替代反射。资源中存在但
/// 尚未迁移的 Filter 不会被伪造：加载时记录缺失并保持链不变。
pub struct FilterManager {
    aliases: RwLock<HashMap<String, String>>,
    factories: RwLock<HashMap<String, Arc<FilterFactory>>>,
}

impl FilterManager {
    /// 使用内置 `druid-filter.properties` 创建管理器。
    ///
    /// 对应 Java 静态初始化块。内置资源解析失败时与 Java 捕获 `Throwable`
    /// 一致：记录错误并保留空别名表，而不是使进程初始化失败。
    #[must_use]
    pub fn new() -> Self {
        let aliases = match Self::load_filter_config() {
            Ok(properties) => Self::aliases_from_properties(&properties),
            Err(error) => {
                tracing::error!(%error, "load filter config error");
                HashMap::new()
            }
        };
        Self {
            aliases: RwLock::new(aliases),
            factories: RwLock::new(HashMap::new()),
        }
    }

    /// 从按加载顺序给出的 properties 文本创建管理器。
    ///
    /// 对应 Java 依次调用 system、类自身、线程 context ClassLoader；后读取的
    /// 资源通过 `putAll` 覆盖同名键。
    ///
    /// # 参数
    /// - `sources`：按低优先级到高优先级排列的 properties 文本。
    ///
    /// # Errors
    ///
    /// Java `Properties.load` 语法中的 Unicode 转义不完整或非法时返回错误。
    pub fn from_property_sources<'a>(
        sources: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, DruidError> {
        let properties = Self::load_filter_config_from_sources(sources)?;
        Ok(Self {
            aliases: RwLock::new(Self::aliases_from_properties(&properties)),
            factories: RwLock::new(HashMap::new()),
        })
    }

    /// 加载 crate 内置的 Java Druid Filter 别名资源。
    ///
    /// 返回完整 properties；只有 `druid.filters.` 前缀会进入别名表。
    ///
    /// # Errors
    ///
    /// 内置 properties 含非法 Unicode 转义时返回参数错误。
    pub fn load_filter_config() -> Result<HashMap<String, String>, DruidError> {
        Self::load_filter_config_from_sources([BUNDLED_FILTER_PROPERTIES])
    }

    /// 按顺序合并多个 Java properties 资源。
    ///
    /// 后出现的资源覆盖先前同名键，对应 `Properties#putAll`。
    ///
    /// # Errors
    ///
    /// 任一 properties 来源含非法 Unicode 转义时返回参数错误。
    pub fn load_filter_config_from_sources<'a>(
        sources: impl IntoIterator<Item = &'a str>,
    ) -> Result<HashMap<String, String>, DruidError> {
        let mut properties = HashMap::new();
        for source in sources {
            properties.extend(parse_java_properties(source)?);
        }
        Ok(properties)
    }

    /// 查询别名对应的 Filter 类名列表。
    ///
    /// # 参数
    /// - `alias`：Java 参数 `alias`；`None` 对应 Java `null`。
    ///
    /// # 返回
    /// 已登记别名返回配置值；未知名称的 Java UTF-16 长度小于 128 时按原类名
    /// 返回，否则返回 `None`。
    #[must_use]
    pub fn get_filter(&self, alias: Option<&str>) -> Option<String> {
        let alias = alias?;
        if let Some(filter) = self.aliases.read().get(alias) {
            return Some(filter.clone());
        }
        (alias.encode_utf16().count() < 128).then(|| alias.to_string())
    }

    /// 增加或覆盖一个 Filter 别名。
    ///
    /// Rust 以显式注册替代 classpath 资源动态发现；键仍保持 Java 的大小写敏感。
    pub fn register_alias(&self, alias: impl Into<String>, filter_class_names: impl Into<String>) {
        self.aliases
            .write()
            .insert(alias.into(), filter_class_names.into());
    }

    /// 注册一个可构造 Java 风格 Filter 三个 trait 视图的工厂。
    ///
    /// # 参数
    /// - `filter_class_name`：Java Filter 完整类名，用于别名解析与重复判定。
    /// - `factory`：每次加载创建一个新 Filter 实例的工厂。
    ///
    /// 同名工厂后注册者覆盖先注册者，对应 classpath 最终可见实现。
    pub fn register_filter<T, F>(&self, filter_class_name: impl Into<String>, factory: F)
    where
        T: BeforeFilter + AfterFilter + ResultSetFilter + 'static,
        F: Fn() -> Result<T, DruidError> + Send + Sync + 'static,
    {
        let class_name = filter_class_name.into();
        let registered_class_name = class_name.clone();
        let factory = move || {
            let filter = Arc::new(factory()?);
            Ok(RegisteredFilter {
                class_name: registered_class_name.clone(),
                before: Arc::clone(&filter) as Arc<dyn BeforeFilter>,
                after: Arc::clone(&filter) as Arc<dyn AfterFilter>,
                result_set: filter,
            })
        };
        self.factories.write().insert(class_name, Arc::new(factory));
    }

    /// 按别名或类名向 `FilterChain` 加载 `Filter`。
    ///
    /// 对应 Java：`FilterManager#loadFilter(List<Filter>, String)`。
    ///
    /// # 参数
    /// - `filter_chain`：接收新实例的链；同一类名按 Java 规则忽略大小写去重。
    /// - `filter_name`：单个别名或类名；空字符串直接返回，逗号展开不做 trim。
    ///
    /// # Errors
    ///
    /// 工厂构造失败时返回带 Java 原消息前缀的错误；未注册类只记录并继续。
    pub fn load_filter(
        &self,
        filter_chain: &mut FilterChain,
        filter_name: &str,
    ) -> Result<(), DruidError> {
        if filter_name.is_empty() {
            return Ok(());
        }

        let Some(filter_class_names) = self.get_filter(Some(filter_name)) else {
            return self.load_direct_filter(filter_chain, filter_name, filter_name);
        };

        for filter_class_name in filter_class_names.split(',') {
            if filter_chain.contains_filter_class_name(filter_class_name) {
                continue;
            }
            self.create_and_add_filter(filter_chain, filter_class_name, filter_name)?;
        }
        Ok(())
    }

    fn load_direct_filter(
        &self,
        filter_chain: &mut FilterChain,
        filter_class_name: &str,
        filter_name: &str,
    ) -> Result<(), DruidError> {
        if filter_chain.contains_filter_class_name(filter_class_name) {
            return Ok(());
        }
        self.create_and_add_filter(filter_chain, filter_class_name, filter_name)
    }

    fn create_and_add_filter(
        &self,
        filter_chain: &mut FilterChain,
        filter_class_name: &str,
        filter_name: &str,
    ) -> Result<(), DruidError> {
        let factory = self.factories.read().get(filter_class_name).cloned();
        let Some(factory) = factory else {
            tracing::error!(filter_class_name, "load filter error, filter not found");
            return Ok(());
        };

        let registered = factory().map_err(|error| {
            DruidError::Other(format!(
                "load managed rdbc driver event listener error. {filter_name}: {error}"
            ))
        })?;
        filter_chain.add_registered_filter(
            registered.class_name,
            registered.before,
            registered.after,
            registered.result_set,
        );
        Ok(())
    }

    fn aliases_from_properties(properties: &HashMap<String, String>) -> HashMap<String, String> {
        properties
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(FILTER_PREFIX)
                    .map(|alias| (alias.to_string(), value.clone()))
            })
            .collect()
    }
}

impl Default for FilterManager {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn parse_java_properties(source: &str) -> Result<HashMap<String, String>, DruidError> {
    let mut properties = HashMap::new();
    let mut logical_line = String::new();

    for physical_line in source.lines() {
        let continuation = physical_line
            .chars()
            .rev()
            .take_while(|character| *character == '\\')
            .count()
            % 2
            == 1;
        let segment = if logical_line.is_empty() {
            physical_line
        } else {
            physical_line.trim_start()
        };
        logical_line.push_str(segment);
        if continuation {
            logical_line.pop();
            continue;
        }
        parse_property_line(&logical_line, &mut properties)?;
        logical_line.clear();
    }
    if !logical_line.is_empty() {
        parse_property_line(&logical_line, &mut properties)?;
    }
    Ok(properties)
}

fn parse_property_line(
    line: &str,
    properties: &mut HashMap<String, String>,
) -> Result<(), DruidError> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return Ok(());
    }

    let mut escaped = false;
    let mut separator = None;
    for (index, character) in trimmed.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == '=' || character == ':' || character.is_whitespace() {
            separator = Some((index, character.len_utf8()));
            break;
        }
    }

    let (key, value) = separator.map_or((trimmed, ""), |(index, width)| {
        let separator_is_whitespace = trimmed[index..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace);
        let mut value = trimmed[index + width..].trim_start();
        if separator_is_whitespace && (value.starts_with('=') || value.starts_with(':')) {
            value = value[1..].trim_start();
        }
        (&trimmed[..index], value)
    });
    properties.insert(unescape_property(key)?, unescape_property(value)?);
    Ok(())
}

fn unescape_property(value: &str) -> Result<String, DruidError> {
    let mut output = Vec::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            let mut encoded = [0; 2];
            output.extend_from_slice(character.encode_utf16(&mut encoded));
            continue;
        }
        match characters.next() {
            Some('t') => output.push(u16::from(b'\t')),
            Some('n') => output.push(u16::from(b'\n')),
            Some('r') => output.push(u16::from(b'\r')),
            Some('f') => output.push(0x000C),
            Some('u') => {
                let digits: String = characters.by_ref().take(4).collect();
                if digits.chars().count() != 4 {
                    return Err(DruidError::InvalidArgument(
                        "malformed Unicode escape in filter properties".to_string(),
                    ));
                }
                let code = u16::from_str_radix(&digits, 16).map_err(|_| {
                    DruidError::InvalidArgument(
                        "malformed Unicode escape in filter properties".to_string(),
                    )
                })?;
                output.push(code);
            }
            Some(escaped) => {
                let mut encoded = [0; 2];
                output.extend_from_slice(escaped.encode_utf16(&mut encoded));
            }
            None => output.push(u16::from(b'\\')),
        }
    }
    Ok(String::from_utf16_lossy(&output))
}
