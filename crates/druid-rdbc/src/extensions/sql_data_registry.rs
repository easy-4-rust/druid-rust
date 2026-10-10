use crate::error::RdbcError;
use crate::extensions::{BufferedSqlInput, SqlDataObject};
use crate::resource::SqlData;
use crate::types::{RdbcObject, RdbcOpaqueObject, RdbcTargetType, RdbcTypeMap};
use std::collections::HashMap;
use std::sync::Arc;

type Constructor = dyn Fn() -> Result<Box<dyn SqlData>, RdbcError> + Send + Sync;

/// 实例级SQLData构造器目录，用Rust工厂适配Java类反射，不另建驱动注册树。
/// 对应 Java: java.sql.SQLInput#readObject 的类型映射构造步骤（Rust扩展）。
#[derive(Default)]
pub struct SqlDataRegistry {
    constructors: HashMap<String, Arc<Constructor>>,
}

impl SqlDataRegistry {
    /// 创建空构造器目录，无参数；返回可由驱动显式配置的实例。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册参数class_name的无参构造器constructor；同名注册替换旧工厂，无返回值。
    /// 构造器必须每次返回新实例；SQLData只要求Send，构造错误原样传播。
    pub fn register<F>(&mut self, class_name: impl Into<String>, constructor: F)
    where
        F: Fn() -> Result<Box<dyn SqlData>, RdbcError> + Send + Sync + 'static,
    {
        self.constructors
            .insert(class_name.into(), Arc::new(constructor));
    }

    /// 按参数map映射一个已读取属性，返回原值或成功初始化的应用对象。
    pub(crate) async fn map_object(
        self: &Arc<Self>,
        value: RdbcObject,
        map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        let RdbcObject::Struct(structure) = value else {
            return Ok(value);
        };
        let type_name = structure.sql_type_name().await?;
        let Some(target) = map.get(&type_name) else {
            return Ok(RdbcObject::Struct(structure));
        };
        let RdbcTargetType::Custom(class_name) = target else {
            return Err(RdbcError::feature_not_supported(
                "SQLInput#readObject(type-map target)",
            ));
        };
        let constructor = self.constructors.get(class_name).ok_or_else(|| {
            RdbcError::feature_not_supported("SQLInput#readObject(unregistered SQLData class)")
        })?;
        // 先构造应用对象，再读取结构属性；构造失败不触碰属性也不发布半成品。
        let mut instance = constructor()?;
        let attributes = structure.attributes().await?;
        // 嵌套SQLInput继承同一显式映射及目录，应用readSQL负责声明顺序。
        let mut input = BufferedSqlInput::with_type_map(attributes, map.clone(), Arc::clone(self));
        instance.read_sql(&mut input, &type_name).await?;
        Ok(RdbcObject::Custom(RdbcOpaqueObject::new(Arc::new(
            SqlDataObject::new(class_name.clone(), instance),
        ))))
    }
}
