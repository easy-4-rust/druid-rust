//! `ResultSet Filter` 调用链。
//!
//! 对应 Java：`com.alibaba.druid.filter.FilterChainImpl` 的 `resultSet_*` 分派。

use super::{
    DruidError, PhysicalResultSet, RdbcArray, RdbcBlob, RdbcCalendarArgument, RdbcCharacterLength,
    RdbcClob, RdbcInputStream, RdbcNClob, RdbcObject, RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml,
    RdbcStreamLength, RdbcTargetType, RdbcTypeMap, RdbcUrl, ResultSetFilter,
    ResultSetFilterContext, ResultSetMetaData, ResultSetStatement, ResultSetUpdate, SqlWarning,
    Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::sync::Arc;

macro_rules! scalar_getter_chain_methods {
    ($(($index:ident, $label:ident, $filter_index:ident, $filter_label:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端调用物理同名重载。")]
            pub fn $index(&mut self, column_index: usize) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical.$physical_index(column_index)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(column_label)
                }
            }
        )+
    };
}

macro_rules! temporal_getter_chain_methods {
    ($(($index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident, $filter_index:ident, $filter_label:ident, $filter_index_calendar:ident, $filter_label_calendar:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端调用无 Calendar 物理重载。")]
            pub fn $index(&mut self, column_index: usize) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical
                        .$physical_index(column_index, &RdbcCalendarArgument::unspecified())
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用无 Calendar 物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(
                        column_label,
                        &RdbcCalendarArgument::unspecified(),
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, Calendar)`，末端保留 Calendar 重载身份。")]
            pub fn $index_calendar(
                &mut self,
                column_index: usize,
                calendar: &RdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index_calendar(self, column_index, calendar)
                } else {
                    self.physical.$physical_index(column_index, calendar)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, Calendar)`，末端保留 Calendar 重载身份。")]
            pub fn $label_calendar(
                &mut self,
                column_label: &str,
                calendar: &RdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label_calendar(self, column_label, calendar)
                } else {
                    self.physical.$physical_label(column_label, calendar)
                }
            }
        )+
    };
}

macro_rules! resource_getter_chain_methods {
    ($(($index:ident, $label:ident, $filter_index:ident, $filter_label:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端返回物理资源句柄。")]
            pub fn $index(&mut self, column_index: usize) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical.$physical_index(column_index)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(column_label)
                }
            }
        )+
    };
}

macro_rules! no_arg_result_set_chain_methods {
    ($(($method:ident, $filter_method:ident, $physical_method:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "()`，末端调用物理方法。")]
            pub fn $method(&mut self) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_method(self)
                } else {
                    self.physical.$physical_method()
                }
            }
        )+
    };
}

macro_rules! i32_arg_result_set_chain_methods {
    ($(($method:ident, $filter_method:ident, $physical_method:ident, $argument:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端保留参数身份。")]
            pub fn $method(&mut self, $argument: i32) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_method(self, $argument)
                } else {
                    self.physical.$physical_method($argument)
                }
            }
        )+
    };
}

macro_rules! scalar_update_chain_methods {
    ($(($index:ident, $label:ident, $filter_index:ident, $filter_label:ident, $ty:ty, $variant:ident, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, ..)`，末端保留 setter 类型身份。")]
            pub fn $index(&mut self, column_index: usize, value: $ty) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index, value)
                } else {
                    self.physical
                        .update_value(column_index, &ResultSetUpdate::$variant(value))
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, ..)`，末端保留标签重载身份。")]
            pub fn $label(&mut self, column_label: &str, value: $ty) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label, value)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant(value),
                    )
                }
            }
        )+
    };
}

macro_rules! resource_update_chain_methods {
    ($(($index:ident, $label:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, ..)`，末端调用物理资源重载。")]
            pub fn $index(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index(self, column_index, value)
                } else {
                    self.physical.$physical_index(column_index, value.as_ref())
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, ..)`，末端调用物理标签资源重载。")]
            pub fn $label(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label(self, column_label, value)
                } else {
                    self.physical.$physical_label(column_label, value.as_ref())
                }
            }
        )+
    };
}

macro_rules! lob_stream_update_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $physical_index:ident,
        $physical_label:ident,
        $ty:ty,
        $length_type:ident,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader)`，末端保留无长度重载。")]
            pub fn $index(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index(self, column_index, value)
                } else {
                    self.physical.$physical_index(
                        column_index,
                        value.as_ref(),
                        $length_type::Unspecified,
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader)`，末端保留标签与无长度重载。")]
            pub fn $label(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label(self, column_label, value)
                } else {
                    self.physical.$physical_label(
                        column_label,
                        value.as_ref(),
                        $length_type::Unspecified,
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader, long)`，末端保留原始 long。")]
            pub fn $index_with_length(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index_with_length(self, column_index, value, length)
                } else {
                    self.physical.$physical_index(
                        column_index,
                        value.as_ref(),
                        $length_type::Long(length),
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader, long)`，末端保留标签与原始 long。")]
            pub fn $label_with_length(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label_with_length(self, column_label, value, length)
                } else {
                    self.physical.$physical_label(
                        column_label,
                        value.as_ref(),
                        $length_type::Long(length),
                    )
                }
            }
        )+
    };
}

macro_rules! stream_update_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_int_length:ident,
        $label_with_int_length:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $resource_field:ident,
        $ty:ty,
        $length_type:ident,
        $variant:ident,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader)`，末端保留无长度描述符。")]
            pub fn $index(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index(self, column_index, value)
                } else {
                    self.physical.update_value(
                        column_index,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Unspecified,
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader)`，末端保留标签与无长度描述符。")]
            pub fn $label(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label(self, column_label, value)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Unspecified,
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader, int)`，末端保留 int 长度描述符。")]
            pub fn $index_with_int_length(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index_with_int_length(self, column_index, value, length)
                } else {
                    self.physical.update_value(
                        column_index,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Int(length),
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader, int)`，末端保留标签与 int 长度描述符。")]
            pub fn $label_with_int_length(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label_with_int_length(self, column_label, value, length)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Int(length),
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader, long)`，末端保留 long 长度描述符。")]
            pub fn $index_with_length(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index_with_length(self, column_index, value, length)
                } else {
                    self.physical.update_value(
                        column_index,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Long(length),
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader, long)`，末端保留标签与 long 长度描述符。")]
            pub fn $label_with_length(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label_with_length(self, column_label, value, length)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Long(length),
                        },
                    )
                }
            }
        )+
    };
}

macro_rules! long_stream_update_chain_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $resource_field:ident,
        $ty:ty,
        $length_type:ident,
        $variant:ident,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader)`，末端保留无长度描述符。")]
            pub fn $index(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index(self, column_index, value)
                } else {
                    self.physical.update_value(
                        column_index,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Unspecified,
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader)`，末端保留标签与无长度描述符。")]
            pub fn $label(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label(self, column_label, value)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Unspecified,
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, stream/reader, long)`，末端保留 long 长度描述符。")]
            pub fn $index_with_length(
                &mut self,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$index_with_length(self, column_index, value, length)
                } else {
                    self.physical.update_value(
                        column_index,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Long(length),
                        },
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, stream/reader, long)`，末端保留标签与 long 长度描述符。")]
            pub fn $label_with_length(
                &mut self,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$label_with_length(self, column_label, value, length)
                } else {
                    self.physical.update_value_by_label(
                        column_label,
                        &ResultSetUpdate::$variant {
                            $resource_field: value,
                            length: $length_type::Long(length),
                        },
                    )
                }
            }
        )+
    };
}

/// 单次 `ResultSet` 操作使用的有位置调用链。
///
/// 每次 `ResultSet` 方法调用都创建新链并从位置 0 开始，等价于 Java
/// `ResultSetProxyImpl#createChain()` 与 `recycleFilterChain(reset)`。
pub struct ResultSetFilterChain<'a> {
    filters: &'a [Arc<dyn ResultSetFilter>],
    position: usize,
    physical: &'a dyn PhysicalResultSet,
    context: &'a ResultSetFilterContext,
    statement: Option<&'a ResultSetStatement>,
}

impl<'a> ResultSetFilterChain<'a> {
    /// 创建从第一个 Filter 开始的单次调用链。
    pub fn new(
        filters: &'a [Arc<dyn ResultSetFilter>],
        physical: &'a dyn PhysicalResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical,
            context,
            statement: None,
        }
    }

    /// 创建携带 `ResultSet#getStatement()` 动态平台对象的单次调用链。
    pub fn new_with_statement(
        filters: &'a [Arc<dyn ResultSetFilter>],
        physical: &'a dyn PhysicalResultSet,
        context: &'a ResultSetFilterContext,
        statement: &'a ResultSetStatement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical,
            context,
            statement: Some(statement),
        }
    }

    /// 继续分派 `ResultSet#next()`，末端调用物理结果集。
    pub fn result_set_next(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_next(self)
        } else {
            self.physical.next()
        }
    }

    /// 继续分派 `ResultSet#close()`，末端调用物理结果集。
    pub fn result_set_close(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_close(self)
        } else {
            self.physical.close()
        }
    }

    /// 继续分派 `ResultSet#getWarnings()`，末端调用物理结果集。
    pub fn result_set_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_warnings(self)
        } else {
            self.physical.warnings()
        }
    }

    /// 继续分派 `ResultSet#clearWarnings()`，末端调用物理结果集。
    pub fn result_set_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_clear_warnings(self)
        } else {
            self.physical.clear_warnings()
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int)`，末端调用物理下标重载。
    pub fn result_set_get_object(&mut self, column_index: usize) -> Result<Value, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object(self, column_index)
        } else {
            self.physical.value(column_index)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String)`，末端调用物理标签重载。
    pub fn result_set_get_object_by_label(
        &mut self,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_by_label(self, column_label)
        } else {
            self.physical.value_by_label(column_label)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int, Map)`，保留 `null` Map。
    pub fn result_set_get_object_with_type_map(
        &mut self,
        column_index: usize,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_with_type_map(self, column_index, type_map)
        } else {
            self.physical.object_with_type_map(column_index, type_map)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String, Map)`，保持标签重载身份。
    pub fn result_set_get_object_by_label_with_type_map(
        &mut self,
        column_label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_by_label_with_type_map(self, column_label, type_map)
        } else {
            self.physical
                .object_by_label_with_type_map(column_label, type_map)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int, Class<T>)`。
    pub fn result_set_get_object_typed(
        &mut self,
        column_index: usize,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_typed(self, column_index, target_type)
        } else {
            self.physical.object_as(column_index, target_type)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String, Class<T>)`。
    pub fn result_set_get_object_typed_by_label(
        &mut self,
        column_label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_typed_by_label(self, column_label, target_type)
        } else {
            self.physical.object_by_label_as(column_label, target_type)
        }
    }

    scalar_getter_chain_methods!(
        (
            result_set_get_string,
            result_set_get_string_by_label,
            result_set_get_string,
            result_set_get_string_by_label,
            string,
            string_by_label,
            Option<String>,
            "getString"
        ),
        (
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            boolean,
            boolean_by_label,
            bool,
            "getBoolean"
        ),
        (
            result_set_get_byte,
            result_set_get_byte_by_label,
            result_set_get_byte,
            result_set_get_byte_by_label,
            byte,
            byte_by_label,
            i8,
            "getByte"
        ),
        (
            result_set_get_short,
            result_set_get_short_by_label,
            result_set_get_short,
            result_set_get_short_by_label,
            short,
            short_by_label,
            i16,
            "getShort"
        ),
        (
            result_set_get_int,
            result_set_get_int_by_label,
            result_set_get_int,
            result_set_get_int_by_label,
            int,
            int_by_label,
            i32,
            "getInt"
        ),
        (
            result_set_get_long,
            result_set_get_long_by_label,
            result_set_get_long,
            result_set_get_long_by_label,
            long,
            long_by_label,
            i64,
            "getLong"
        ),
        (
            result_set_get_float,
            result_set_get_float_by_label,
            result_set_get_float,
            result_set_get_float_by_label,
            float,
            float_by_label,
            f32,
            "getFloat"
        ),
        (
            result_set_get_double,
            result_set_get_double_by_label,
            result_set_get_double,
            result_set_get_double_by_label,
            double,
            double_by_label,
            f64,
            "getDouble"
        ),
        (
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            bytes,
            bytes_by_label,
            Option<Vec<u8>>,
            "getBytes"
        ),
        (
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            n_string,
            n_string_by_label,
            Option<String>,
            "getNString"
        ),
    );

    /// 继续分派 Java `ResultSet#getBigDecimal(int)`。
    pub fn result_set_get_big_decimal(
        &mut self,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal(self, column_index)
        } else {
            self.physical.big_decimal(column_index, None)
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(String)`。
    pub fn result_set_get_big_decimal_by_label(
        &mut self,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_by_label(self, column_label)
        } else {
            self.physical.big_decimal_by_label(column_label, None)
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(int, int)`。
    pub fn result_set_get_big_decimal_with_scale(
        &mut self,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_with_scale(self, column_index, scale)
        } else {
            self.physical.big_decimal(column_index, Some(scale))
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(String, int)`。
    pub fn result_set_get_big_decimal_by_label_with_scale(
        &mut self,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_by_label_with_scale(self, column_label, scale)
        } else {
            self.physical
                .big_decimal_by_label(column_label, Some(scale))
        }
    }

    temporal_getter_chain_methods!(
        (
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            date,
            date_by_label,
            NaiveDate,
            "getDate"
        ),
        (
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            time,
            time_by_label,
            NaiveTime,
            "getTime"
        ),
        (
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            timestamp,
            timestamp_by_label,
            NaiveDateTime,
            "getTimestamp"
        ),
    );

    resource_getter_chain_methods!(
        (
            result_set_get_ref,
            result_set_get_ref_by_label,
            result_set_get_ref,
            result_set_get_ref_by_label,
            reference,
            reference_by_label,
            RdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            result_set_get_blob,
            result_set_get_blob_by_label,
            blob,
            blob_by_label,
            RdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            result_set_get_clob,
            result_set_get_clob_by_label,
            clob,
            clob_by_label,
            RdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            result_set_get_array,
            result_set_get_array_by_label,
            array,
            array_by_label,
            RdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            result_set_get_url,
            result_set_get_url_by_label,
            url,
            url_by_label,
            RdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            row_id,
            row_id_by_label,
            RdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            n_clob,
            n_clob_by_label,
            RdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            sql_xml,
            sql_xml_by_label,
            RdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            ascii_stream,
            ascii_stream_by_label,
            RdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            unicode_stream,
            unicode_stream_by_label,
            RdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            binary_stream,
            binary_stream_by_label,
            RdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            character_stream,
            character_stream_by_label,
            RdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            n_character_stream,
            n_character_stream_by_label,
            RdbcReader,
            "getNCharacterStream"
        ),
    );

    no_arg_result_set_chain_methods!(
        (result_set_was_null, result_set_was_null, was_null, bool, "wasNull"),
        (result_set_previous, result_set_previous, previous, bool, "previous"),
        (result_set_is_before_first, result_set_is_before_first, is_before_first, bool, "isBeforeFirst"),
        (result_set_is_after_last, result_set_is_after_last, is_after_last, bool, "isAfterLast"),
        (result_set_is_first, result_set_is_first, is_first, bool, "isFirst"),
        (result_set_is_last, result_set_is_last, is_last, bool, "isLast"),
        (result_set_before_first, result_set_before_first, before_first, (), "beforeFirst"),
        (result_set_after_last, result_set_after_last, after_last, (), "afterLast"),
        (result_set_first, result_set_first, first, bool, "first"),
        (result_set_last, result_set_last, last, bool, "last"),
        (result_set_get_row, result_set_get_row, row, i32, "getRow"),
        (result_set_get_fetch_direction, result_set_get_fetch_direction, fetch_direction, i32, "getFetchDirection"),
        (result_set_get_fetch_size, result_set_get_fetch_size, fetch_size, i32, "getFetchSize"),
        (result_set_get_type, result_set_get_type, result_set_type, i32, "getType"),
        (result_set_get_concurrency, result_set_get_concurrency, concurrency, i32, "getConcurrency"),
        (result_set_get_holdability, result_set_get_holdability, holdability, i32, "getHoldability"),
        (result_set_get_cursor_name, result_set_get_cursor_name, cursor_name, Option<String>, "getCursorName"),
        (result_set_row_updated, result_set_row_updated, row_updated, bool, "rowUpdated"),
        (result_set_row_inserted, result_set_row_inserted, row_inserted, bool, "rowInserted"),
        (result_set_row_deleted, result_set_row_deleted, row_deleted, bool, "rowDeleted"),
        (result_set_insert_row, result_set_insert_row, insert_row, (), "insertRow"),
        (result_set_update_row, result_set_update_row, update_row, (), "updateRow"),
        (result_set_delete_row, result_set_delete_row, delete_row, (), "deleteRow"),
        (result_set_refresh_row, result_set_refresh_row, refresh_row, (), "refreshRow"),
        (
            result_set_cancel_row_updates,
            result_set_cancel_row_updates,
            cancel_row_updates,
            (),
            "cancelRowUpdates"
        ),
        (
            result_set_move_to_insert_row,
            result_set_move_to_insert_row,
            move_to_insert_row,
            (),
            "moveToInsertRow"
        ),
        (
            result_set_move_to_current_row,
            result_set_move_to_current_row,
            move_to_current_row,
            (),
            "moveToCurrentRow"
        ),
    );

    i32_arg_result_set_chain_methods!(
        (
            result_set_absolute,
            result_set_absolute,
            absolute,
            row,
            bool,
            "absolute"
        ),
        (
            result_set_relative,
            result_set_relative,
            relative,
            rows,
            bool,
            "relative"
        ),
        (
            result_set_set_fetch_direction,
            result_set_set_fetch_direction,
            set_fetch_direction,
            direction,
            (),
            "setFetchDirection"
        ),
        (
            result_set_set_fetch_size,
            result_set_set_fetch_size,
            set_fetch_size,
            rows,
            (),
            "setFetchSize"
        ),
    );

    /// 继续分派 Java `ResultSet#updateNull(int)`。
    pub fn result_set_update_null(&mut self, column_index: usize) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_null(self, column_index)
        } else {
            self.physical
                .update_value(column_index, &ResultSetUpdate::Null)
        }
    }

    /// 继续分派 Java `ResultSet#updateNull(String)`。
    pub fn result_set_update_null_by_label(
        &mut self,
        column_label: &str,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_null_by_label(self, column_label)
        } else {
            self.physical
                .update_value_by_label(column_label, &ResultSetUpdate::Null)
        }
    }

    scalar_update_chain_methods!(
        (result_set_update_boolean, result_set_update_boolean_by_label, result_set_update_boolean, result_set_update_boolean_by_label, bool, Boolean, "updateBoolean"),
        (result_set_update_byte, result_set_update_byte_by_label, result_set_update_byte, result_set_update_byte_by_label, i8, Byte, "updateByte"),
        (result_set_update_short, result_set_update_short_by_label, result_set_update_short, result_set_update_short_by_label, i16, Short, "updateShort"),
        (result_set_update_int, result_set_update_int_by_label, result_set_update_int, result_set_update_int_by_label, i32, Int, "updateInt"),
        (result_set_update_long, result_set_update_long_by_label, result_set_update_long, result_set_update_long_by_label, i64, Long, "updateLong"),
        (result_set_update_float, result_set_update_float_by_label, result_set_update_float, result_set_update_float_by_label, f32, Float, "updateFloat"),
        (result_set_update_double, result_set_update_double_by_label, result_set_update_double, result_set_update_double_by_label, f64, Double, "updateDouble"),
        (result_set_update_big_decimal, result_set_update_big_decimal_by_label, result_set_update_big_decimal, result_set_update_big_decimal_by_label, Option<BigDecimal>, BigDecimal, "updateBigDecimal"),
        (result_set_update_string, result_set_update_string_by_label, result_set_update_string, result_set_update_string_by_label, Option<String>, String, "updateString"),
        (result_set_update_n_string, result_set_update_n_string_by_label, result_set_update_n_string, result_set_update_n_string_by_label, Option<String>, NString, "updateNString"),
        (result_set_update_bytes, result_set_update_bytes_by_label, result_set_update_bytes, result_set_update_bytes_by_label, Option<Vec<u8>>, Bytes, "updateBytes"),
        (result_set_update_date, result_set_update_date_by_label, result_set_update_date, result_set_update_date_by_label, Option<NaiveDate>, Date, "updateDate"),
        (result_set_update_time, result_set_update_time_by_label, result_set_update_time, result_set_update_time_by_label, Option<NaiveTime>, Time, "updateTime"),
        (result_set_update_timestamp, result_set_update_timestamp_by_label, result_set_update_timestamp, result_set_update_timestamp_by_label, Option<NaiveDateTime>, Timestamp, "updateTimestamp"),
    );

    /// 继续分派 Java `ResultSet#updateObject(int, Object)`。
    pub fn result_set_update_object(
        &mut self,
        column_index: usize,
        value: RdbcObject,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_object(self, column_index, value)
        } else {
            self.physical
                .update_value(column_index, &ResultSetUpdate::Object(value))
        }
    }

    /// 继续分派 Java `ResultSet#updateObject(String, Object)`。
    pub fn result_set_update_object_by_label(
        &mut self,
        column_label: &str,
        value: RdbcObject,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_object_by_label(self, column_label, value)
        } else {
            self.physical
                .update_value_by_label(column_label, &ResultSetUpdate::Object(value))
        }
    }

    /// 继续分派 Java `ResultSet#updateObject(int, Object, int)`。
    pub fn result_set_update_object_with_scale_or_length(
        &mut self,
        column_index: usize,
        value: RdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_object_with_scale_or_length(
                self,
                column_index,
                value,
                scale_or_length,
            )
        } else {
            self.physical.update_value(
                column_index,
                &ResultSetUpdate::ObjectWithScaleOrLength {
                    value,
                    scale_or_length,
                },
            )
        }
    }

    /// 继续分派 Java `ResultSet#updateObject(String, Object, int)`。
    pub fn result_set_update_object_by_label_with_scale_or_length(
        &mut self,
        column_label: &str,
        value: RdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_update_object_by_label_with_scale_or_length(
                self,
                column_label,
                value,
                scale_or_length,
            )
        } else {
            self.physical.update_value_by_label(
                column_label,
                &ResultSetUpdate::ObjectWithScaleOrLength {
                    value,
                    scale_or_length,
                },
            )
        }
    }

    resource_update_chain_methods!(
        (
            result_set_update_reference,
            result_set_update_reference_by_label,
            update_reference,
            update_reference_by_label,
            RdbcRef,
            "updateRef"
        ),
        (
            result_set_update_blob,
            result_set_update_blob_by_label,
            update_blob,
            update_blob_by_label,
            RdbcBlob,
            "updateBlob"
        ),
        (
            result_set_update_clob,
            result_set_update_clob_by_label,
            update_clob,
            update_clob_by_label,
            RdbcClob,
            "updateClob"
        ),
        (
            result_set_update_array,
            result_set_update_array_by_label,
            update_array,
            update_array_by_label,
            RdbcArray,
            "updateArray"
        ),
        (
            result_set_update_row_id,
            result_set_update_row_id_by_label,
            update_row_id,
            update_row_id_by_label,
            RdbcRowId,
            "updateRowId"
        ),
        (
            result_set_update_n_clob,
            result_set_update_n_clob_by_label,
            update_n_clob,
            update_n_clob_by_label,
            RdbcNClob,
            "updateNClob"
        ),
        (
            result_set_update_sql_xml,
            result_set_update_sql_xml_by_label,
            update_sql_xml,
            update_sql_xml_by_label,
            RdbcSqlXml,
            "updateSQLXML"
        ),
    );

    lob_stream_update_chain_methods!(
        (
            result_set_update_blob_stream,
            result_set_update_blob_stream_by_label,
            result_set_update_blob_stream_with_length,
            result_set_update_blob_stream_by_label_with_length,
            update_blob_stream,
            update_blob_stream_by_label,
            RdbcInputStream,
            RdbcStreamLength,
            "updateBlob"
        ),
        (
            result_set_update_clob_reader,
            result_set_update_clob_reader_by_label,
            result_set_update_clob_reader_with_length,
            result_set_update_clob_reader_by_label_with_length,
            update_clob_reader,
            update_clob_reader_by_label,
            RdbcReader,
            RdbcCharacterLength,
            "updateClob"
        ),
        (
            result_set_update_n_clob_reader,
            result_set_update_n_clob_reader_by_label,
            result_set_update_n_clob_reader_with_length,
            result_set_update_n_clob_reader_by_label_with_length,
            update_n_clob_reader,
            update_n_clob_reader_by_label,
            RdbcReader,
            RdbcCharacterLength,
            "updateNClob"
        ),
    );

    stream_update_chain_methods!(
        (
            result_set_update_ascii_stream,
            result_set_update_ascii_stream_by_label,
            result_set_update_ascii_stream_with_int_length,
            result_set_update_ascii_stream_by_label_with_int_length,
            result_set_update_ascii_stream_with_length,
            result_set_update_ascii_stream_by_label_with_length,
            stream,
            RdbcInputStream,
            RdbcStreamLength,
            AsciiStream,
            "updateAsciiStream"
        ),
        (
            result_set_update_binary_stream,
            result_set_update_binary_stream_by_label,
            result_set_update_binary_stream_with_int_length,
            result_set_update_binary_stream_by_label_with_int_length,
            result_set_update_binary_stream_with_length,
            result_set_update_binary_stream_by_label_with_length,
            stream,
            RdbcInputStream,
            RdbcStreamLength,
            BinaryStream,
            "updateBinaryStream"
        ),
        (
            result_set_update_character_stream,
            result_set_update_character_stream_by_label,
            result_set_update_character_stream_with_int_length,
            result_set_update_character_stream_by_label_with_int_length,
            result_set_update_character_stream_with_length,
            result_set_update_character_stream_by_label_with_length,
            reader,
            RdbcReader,
            RdbcCharacterLength,
            CharacterStream,
            "updateCharacterStream"
        ),
    );

    long_stream_update_chain_methods!((
        result_set_update_n_character_stream,
        result_set_update_n_character_stream_by_label,
        result_set_update_n_character_stream_with_length,
        result_set_update_n_character_stream_by_label_with_length,
        reader,
        RdbcReader,
        RdbcCharacterLength,
        NCharacterStream,
        "updateNCharacterStream"
    ));

    /// 继续分派 Java `ResultSet#findColumn(String)`。
    pub fn result_set_find_column(&mut self, column_label: &str) -> Result<usize, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_find_column(self, column_label)
        } else {
            self.physical.find_column(column_label)
        }
    }

    /// 继续分派 Java `ResultSet#isClosed()`。
    pub fn result_set_is_closed(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_is_closed(self)
        } else {
            Ok(self.physical.is_closed())
        }
    }

    /// 继续分派 Java `ResultSet#getMetaData()`，末端返回物理 metadata 句柄。
    pub fn result_set_get_meta_data(&mut self) -> Result<ResultSetMetaData, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_meta_data(self)
        } else {
            self.physical.meta_data()
        }
    }

    /// 继续分派 Java `ResultSet#getStatement()`，末端返回共享动态平台句柄。
    pub fn result_set_get_statement(&mut self) -> Result<ResultSetStatement, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_statement(self)
        } else {
            self.statement
                .cloned()
                .ok_or(DruidError::UnsupportedOperation {
                    operation: "result_set_get_statement_without_platform_object",
                })
        }
    }

    /// 返回本结果集共享的 Filter 上下文。
    pub fn context(&self) -> &ResultSetFilterContext {
        self.context
    }

    /// 直接读取当前物理行的 1-based 列值，不重新进入 Filter 链。
    ///
    /// 对应 Java：`ResultSetProxy#getResultSetRaw().getObject(columnIndex)`；
    /// `WallFilter#resultSet_next` 用它向租户回调报告真实列值。
    pub fn raw_value(&self, column_index: usize) -> Result<Value, DruidError> {
        self.physical.value(column_index)
    }
}
