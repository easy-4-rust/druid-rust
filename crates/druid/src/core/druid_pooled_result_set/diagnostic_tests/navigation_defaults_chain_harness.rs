use crate::core::{
    DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, ResultSetPropertyOperation,
    ResultSetPropertyValue,
};
use druid_rdbc::result_set::{ResultSet, ResultSetNavigation};
use druid_rdbc::statement::FetchDirection;
use std::sync::Arc;
/// 将原测试入口映射到实际标准过滤器链，保留所有原值和原生日志观察。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct NavigationDefaultsChainHarness {
    chain: FilterChain,
}
impl NavigationDefaultsChainHarness {
    /// 创建标准链桥接；无参数，返回空过滤器链。
    pub(super) fn new() -> Self {
        Self {
            chain: FilterChain::new(),
        }
    }
    /// 添加原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.chain.add_result_set(filter);
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_previous(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::Previous, false)
            .await?;
        Ok(present)
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_before_first(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::BeforeFirst, false)
            .await?;
        let _ = present;
        Ok(())
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_after_last(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::AfterLast, false)
            .await?;
        let _ = present;
        Ok(())
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_first(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::First, false)
            .await?;
        Ok(present)
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_last(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::Last, false)
            .await?;
        Ok(present)
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_absolute(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        row: i32,
    ) -> Result<bool, DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::Absolute(row), false)
            .await?;
        Ok(present)
    }
    /// 委托原导航；参数为驱动、上下文及原导航参数，返回原布尔或完成状态。
    pub(super) async fn result_set_relative(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        row: i32,
    ) -> Result<bool, DruidError> {
        let (present, _) = self
            .chain
            .standard_result_set_navigation(raw, context, ResultSetNavigation::Relative(row), false)
            .await?;
        Ok(present)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_was_null(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::WasNull,
            &mut || {
                raw.was_null()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_is_before_first(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::IsBeforeFirst,
            &mut || {
                raw.is_before_first()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_is_after_last(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::IsAfterLast,
            &mut || {
                raw.is_after_last()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_is_first(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::IsFirst,
            &mut || {
                raw.is_first()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_is_last(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::IsLast,
            &mut || {
                raw.is_last()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::Row,
            &mut || {
                raw.row()
                    .map(ResultSetPropertyValue::Integer)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_integer().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_fetch_direction(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::FetchDirection,
            &mut || {
                raw.fetch_direction()
                    .map(ResultSetPropertyValue::FetchDirection)
                    .map_err(DruidError::from)
            },
        )?;
        Ok(value
            .into_direction()
            .map_err(DruidError::from)?
            .rdbc_code())
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_fetch_size(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::FetchSize,
            &mut || {
                raw.fetch_size()
                    .map(ResultSetPropertyValue::Integer)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_integer().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_type(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::ResultSetType,
            &mut || {
                raw.result_set_type()
                    .map(ResultSetPropertyValue::ResultSetType)
                    .map_err(DruidError::from)
            },
        )?;
        Ok(value
            .into_result_set_type()
            .map_err(DruidError::from)?
            .rdbc_code())
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_concurrency(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::Concurrency,
            &mut || {
                raw.concurrency()
                    .map(ResultSetPropertyValue::Concurrency)
                    .map_err(DruidError::from)
            },
        )?;
        Ok(value
            .into_concurrency()
            .map_err(DruidError::from)?
            .rdbc_code())
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_holdability(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<i32, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::Holdability,
            &mut || {
                raw.holdability()
                    .map(ResultSetPropertyValue::Holdability)
                    .map_err(DruidError::from)
            },
        )?;
        Ok(value
            .into_holdability()
            .map_err(DruidError::from)?
            .rdbc_code())
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_get_cursor_name(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<Option<String>, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::CursorName,
            &mut || {
                raw.cursor_name()
                    .map(ResultSetPropertyValue::String)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_string().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_row_updated(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::RowUpdated,
            &mut || {
                raw.row_updated()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_row_inserted(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::RowInserted,
            &mut || {
                raw.row_inserted()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_row_deleted(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::RowDeleted,
            &mut || {
                raw.row_deleted()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原属性读取；参数为驱动与上下文，返回原类型属性或精确错误。
    pub(super) fn result_set_is_closed(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::IsClosed,
            &mut || {
                raw.is_closed()
                    .map(ResultSetPropertyValue::Boolean)
                    .map_err(DruidError::from)
            },
        )?;
        value.into_boolean().map_err(DruidError::from)
    }
    /// 委托原方向设置；参数为驱动、上下文和 JDBC 方向整数，返回完成状态。
    pub(super) fn result_set_set_fetch_direction(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        direction: i32,
    ) -> Result<(), DruidError> {
        let direction = FetchDirection::try_from(direction).map_err(DruidError::from)?;
        self.chain
            .standard_result_set_property(
                context,
                ResultSetPropertyOperation::SetFetchDirection(direction),
                &mut || {
                    raw.set_fetch_direction(direction)
                        .map(|()| ResultSetPropertyValue::Unit)
                        .map_err(DruidError::from)
                },
            )?
            .into_unit()
            .map_err(DruidError::from)
    }
    /// 委托原提取大小；参数为驱动、上下文与有符号大小，返回完成状态。
    pub(super) fn result_set_set_fetch_size(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        size: i32,
    ) -> Result<(), DruidError> {
        self.chain
            .standard_result_set_property(
                context,
                ResultSetPropertyOperation::SetFetchSize(size),
                &mut || {
                    raw.set_fetch_size(size)
                        .map(|()| ResultSetPropertyValue::Unit)
                        .map_err(DruidError::from)
                },
            )?
            .into_unit()
            .map_err(DruidError::from)
    }
    /// 委托原标签定位；参数为驱动、上下文与标签，返回原一基下标。
    pub(super) fn result_set_find_column(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
        label: &str,
    ) -> Result<usize, DruidError> {
        let value = self.chain.standard_result_set_property(
            context,
            ResultSetPropertyOperation::FindColumn(label),
            &mut || {
                raw.find_column(label)
                    .map(ResultSetPropertyValue::Column)
                    .map_err(DruidError::from)
            },
        )?;
        Ok(value.into_column().map_err(DruidError::from)?.get())
    }
}
