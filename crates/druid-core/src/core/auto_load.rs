//! 对应 Java：`com.alibaba.druid.filter.AutoLoad`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/AutoLoad.java`。

use super::{DruidError, FilterChain, FilterManager};

/// Filter 的编译期自动发现元数据。
///
/// Java 使用运行时 `@AutoLoad` 注解筛选 `ServiceLoader<Filter>` provider。
/// Rust 没有类注解和 classpath ServiceLoader，因此以 `inventory` 保存同一
/// `value=true` 元数据，并由注册函数把具体 Filter factory 安装到
/// [`FilterManager`]。扩展 crate 可在不修改 druid 核心清单的情况下提交条目。
///
/// `order` 是 Rust 宿主为 Java provider-file 顺序提供的显式替代；相同 order
/// 按类名排序，避免 inventory 未定义迭代顺序影响 Filter 调用结果。
pub struct AutoLoad {
    value: bool,
    filter_class_name: &'static str,
    order: i32,
    register: fn(&FilterManager),
}

impl AutoLoad {
    /// 创建默认顺序的自动加载描述符。
    ///
    /// # 参数
    ///
    /// - `filter_class_name`：用于去重和诊断的 Java 风格稳定类名。
    /// - `value`：对应 Java `AutoLoad#value()`。
    /// - `register`：向 manager 注册具体 Filter factory 的无捕获函数。
    #[must_use]
    pub const fn new(
        filter_class_name: &'static str,
        value: bool,
        register: fn(&FilterManager),
    ) -> Self {
        Self {
            value,
            filter_class_name,
            order: 0,
            register,
        }
    }

    /// 创建带显式 provider 顺序的自动加载描述符。
    #[must_use]
    pub const fn with_order(
        filter_class_name: &'static str,
        value: bool,
        order: i32,
        register: fn(&FilterManager),
    ) -> Self {
        Self {
            value,
            filter_class_name,
            order,
            register,
        }
    }

    /// 返回是否允许自动加载，对应 Java annotation 的 `value()`。
    #[must_use]
    pub const fn value(&self) -> bool {
        self.value
    }

    /// 返回稳定 Filter 类名。
    #[must_use]
    pub const fn filter_class_name(&self) -> &'static str {
        self.filter_class_name
    }

    /// 返回显式 provider 顺序。
    #[must_use]
    pub const fn order(&self) -> i32 {
        self.order
    }

    /// 将所有启用的 inventory provider 加入 canonical Filter 链。
    ///
    /// Java 在显式 Filter 完成 `init` 后才执行 `initFromSPIServiceLoader`；
    /// 调用方必须保持这一时序，自动 provider 不重复执行生命周期初始化。
    ///
    /// # Errors
    ///
    /// provider 注册的 factory 构造失败时，保留 `FilterManager` 的错误语义。
    pub(crate) fn load_registered(
        manager: &FilterManager,
        filter_chain: &mut FilterChain,
    ) -> Result<(), DruidError> {
        let mut providers: Vec<&'static Self> = inventory::iter::<Self>
            .into_iter()
            .filter(|provider| provider.value)
            .collect();
        providers.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.filter_class_name.cmp(right.filter_class_name))
        });

        for provider in providers {
            tracing::info!(
                filter_class_name = provider.filter_class_name,
                "load filter from inventory"
            );
            (provider.register)(manager);
            manager.load_filter(filter_chain, provider.filter_class_name)?;
        }
        Ok(())
    }
}

inventory::collect!(AutoLoad);
