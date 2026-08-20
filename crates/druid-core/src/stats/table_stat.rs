//! 对应 Java 类：`com.alibaba.druid.stat.TableStat`。

use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

const FNV_BASIC: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 单表 SQL 操作计数。
///
/// 对应 Java: `com.alibaba.druid.stat.TableStat`。计数使用 `i32` wrapping
/// 增加，保留 Java int 溢出语义。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableStat {
    select_count: i32,
    update_count: i32,
    delete_count: i32,
    insert_count: i32,
    drop_count: i32,
    merge_count: i32,
    create_count: i32,
    alter_count: i32,
    create_index_count: i32,
    drop_index_count: i32,
    referenced_count: i32,
    add_count: i32,
    add_partition_count: i32,
    analyze_count: i32,
}

macro_rules! table_counter {
    ($getter:ident, $increment:ident, $field:ident) => {
        #[doc = "返回对应 Java `TableStat` 计数。"]
        #[must_use]
        pub const fn $getter(&self) -> i32 {
            self.$field
        }

        #[doc = "以 Java int wrapping 语义增加对应计数。"]
        pub fn $increment(&mut self) {
            self.$field = self.$field.wrapping_add(1);
        }
    };
}

impl TableStat {
    table_counter!(
        referenced_count,
        increment_referenced_count,
        referenced_count
    );
    table_counter!(
        drop_index_count,
        increment_drop_index_count,
        drop_index_count
    );
    table_counter!(add_count, increment_add_count, add_count);
    table_counter!(
        add_partition_count,
        increment_add_partition_count,
        add_partition_count
    );
    table_counter!(
        create_index_count,
        increment_create_index_count,
        create_index_count
    );
    table_counter!(alter_count, increment_alter_count, alter_count);
    table_counter!(create_count, increment_create_count, create_count);
    table_counter!(merge_count, increment_merge_count, merge_count);
    table_counter!(drop_count, increment_drop_count, drop_count);
    table_counter!(select_count, increment_select_count, select_count);
    table_counter!(update_count, increment_update_count, update_count);
    table_counter!(delete_count, increment_delete_count, delete_count);
    table_counter!(insert_count, increment_insert_count, insert_count);
    table_counter!(analyze_count, increment_analyze_count, analyze_count);

    /// 设置 dropCount。
    pub fn set_drop_count(&mut self, value: i32) {
        self.drop_count = value;
    }

    /// 设置 selectCount。
    pub fn set_select_count(&mut self, value: i32) {
        self.select_count = value;
    }

    /// 设置 updateCount。
    pub fn set_update_count(&mut self, value: i32) {
        self.update_count = value;
    }

    /// 设置 deleteCount。
    pub fn set_delete_count(&mut self, value: i32) {
        self.delete_count = value;
    }

    /// 设置 insertCount。
    pub fn set_insert_count(&mut self, value: i32) {
        self.insert_count = value;
    }
}

impl Display for TableStat {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        for (count, name) in [
            (self.merge_count, "Merge"),
            (self.insert_count, "Insert"),
            (self.update_count, "Update"),
            (self.select_count, "Select"),
            (self.delete_count, "Delete"),
            (self.drop_count, "Drop"),
            (self.create_count, "Create"),
            (self.alter_count, "Alter"),
            (self.create_index_count, "CreateIndex"),
            (self.drop_index_count, "DropIndex"),
            (self.add_count, "Add"),
            (self.add_partition_count, "AddPartition"),
            (self.analyze_count, "Analyze"),
        ] {
            if count > 0 {
                formatter.write_str(name)?;
            }
        }
        Ok(())
    }
}

/// 表名及其 Java FNV-1a 64 位标识。
///
/// 对应 Java: `TableStat.Name`。
#[derive(Debug, Clone)]
pub struct TableStatName {
    name: String,
    hash_code_64: u64,
}

impl TableStatName {
    /// 按 Java normalized/lower FNV 规则创建。
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let hash_code_64 = normalized_hash(&name);
        Self { name, hash_code_64 }
    }

    /// 使用外部预计算 hash 创建。
    #[must_use]
    pub fn with_hash(name: impl Into<String>, hash_code_64: u64) -> Self {
        Self {
            name: name.into(),
            hash_code_64,
        }
    }

    /// 返回原始名称。
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回 64 位 hash。
    #[must_use]
    pub const fn hash_code_64(&self) -> u64 {
        self.hash_code_64
    }
}

impl PartialEq for TableStatName {
    fn eq(&self, other: &Self) -> bool {
        self.hash_code_64 == other.hash_code_64
    }
}

impl Eq for TableStatName {}

impl Hash for TableStatName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let java_hash = (self.hash_code_64 ^ (self.hash_code_64 >> 32)) as u32;
        state.write_u32(java_hash);
    }
}

impl Display for TableStatName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&normalize_identifier(&self.name))
    }
}

/// 两个列之间的关系。
///
/// 对应 Java: `TableStat.Relationship`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableStatRelationship {
    left: TableStatColumn,
    right: TableStatColumn,
    operator: String,
}

impl TableStatRelationship {
    /// 创建列关系。
    #[must_use]
    pub fn new(left: TableStatColumn, right: TableStatColumn, operator: impl Into<String>) -> Self {
        Self {
            left,
            right,
            operator: operator.into(),
        }
    }

    /// 返回左列。
    #[must_use]
    pub const fn left(&self) -> &TableStatColumn {
        &self.left
    }

    /// 返回右列。
    #[must_use]
    pub const fn right(&self) -> &TableStatColumn {
        &self.right
    }

    /// 返回操作符。
    #[must_use]
    pub fn operator(&self) -> &str {
        &self.operator
    }
}

impl Display for TableStatRelationship {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {} {}", self.left, self.operator, self.right)
    }
}

/// 单列过滤条件。
///
/// 对应 Java: `TableStat.Condition`。相等/hash 故意忽略 values。
#[derive(Debug, Clone)]
pub struct TableStatCondition {
    column: TableStatColumn,
    operator: String,
    values: Vec<Value>,
}

impl TableStatCondition {
    /// 创建空值列表条件。
    #[must_use]
    pub fn new(column: TableStatColumn, operator: impl Into<String>) -> Self {
        Self {
            column,
            operator: operator.into(),
            values: Vec::new(),
        }
    }

    /// 返回列。
    #[must_use]
    pub const fn column(&self) -> &TableStatColumn {
        &self.column
    }

    /// 返回操作符。
    #[must_use]
    pub fn operator(&self) -> &str {
        &self.operator
    }

    /// 返回值列表。
    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// 追加条件值。
    pub fn add_value(&mut self, value: Value) {
        self.values.push(value);
    }
}

impl PartialEq for TableStatCondition {
    fn eq(&self, other: &Self) -> bool {
        self.column == other.column && self.operator == other.operator
    }
}

impl Eq for TableStatCondition {}

impl Hash for TableStatCondition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.column.hash(state);
        self.operator.hash(state);
    }
}

impl Display for TableStatCondition {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.column, self.operator)?;
        match self.values.as_slice() {
            [] => Ok(()),
            [value] => write!(formatter, " {}", java_value(value, false)),
            values => {
                formatter.write_str(" (")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    formatter.write_str(&java_value(value, true))?;
                }
                formatter.write_str(")")
            }
        }
    }
}

/// 表列及其访问位置标志。
///
/// 对应 Java: `TableStat.Column`。
#[derive(Debug)]
pub struct TableStatColumn {
    table: Option<String>,
    name: String,
    hash_code_64: u64,
    where_column: bool,
    select: bool,
    group_by: bool,
    having: bool,
    join: bool,
    primary_key: bool,
    unique: bool,
    update: bool,
    attributes: HashMap<String, Value>,
    full_name: OnceLock<String>,
    data_type: Option<String>,
}

impl Clone for TableStatColumn {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
            name: self.name.clone(),
            hash_code_64: self.hash_code_64,
            where_column: self.where_column,
            select: self.select,
            group_by: self.group_by,
            having: self.having,
            join: self.join,
            primary_key: self.primary_key,
            unique: self.unique,
            update: self.update,
            attributes: self.attributes.clone(),
            full_name: OnceLock::new(),
            data_type: self.data_type.clone(),
        }
    }
}

impl TableStatColumn {
    /// 创建列并按 table.name 计算 Java FNV hash。
    #[must_use]
    pub fn new(table: Option<String>, name: impl Into<String>) -> Self {
        let name = name.into();
        let hash_code_64 = qualified_hash(table.as_deref(), &name);
        Self::with_hash(table, name, hash_code_64)
    }

    /// 使用外部预计算 hash 创建列。
    #[must_use]
    pub fn with_hash(table: Option<String>, name: impl Into<String>, hash_code_64: u64) -> Self {
        Self {
            table,
            name: name.into(),
            hash_code_64,
            where_column: false,
            select: false,
            group_by: false,
            having: false,
            join: false,
            primary_key: false,
            unique: false,
            update: false,
            attributes: HashMap::new(),
            full_name: OnceLock::new(),
            data_type: None,
        }
    }

    /// 返回 table。
    #[must_use]
    pub fn table(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 返回并缓存 table.name。
    #[must_use]
    pub fn full_name(&self) -> &str {
        self.full_name.get_or_init(|| {
            self.table.as_ref().map_or_else(
                || self.name.clone(),
                |table| format!("{table}.{}", self.name),
            )
        })
    }

    /// 返回列名。
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回 64 位 hash。
    #[must_use]
    pub const fn hash_code_64(&self) -> u64 {
        self.hash_code_64
    }

    /// 返回 where 标志。
    #[must_use]
    pub const fn is_where(&self) -> bool {
        self.where_column
    }

    /// 设置 where 标志。
    pub fn set_where(&mut self, value: bool) {
        self.where_column = value;
    }

    /// 返回 select 标志。
    #[must_use]
    pub const fn is_select(&self) -> bool {
        self.select
    }

    /// 设置 select 标志；对应 Java 拼写错误的 `setSelec`。
    pub fn set_selec(&mut self, value: bool) {
        self.select = value;
    }

    /// 返回 groupBy 标志。
    #[must_use]
    pub const fn is_group_by(&self) -> bool {
        self.group_by
    }

    /// 设置 groupBy 标志。
    pub fn set_group_by(&mut self, value: bool) {
        self.group_by = value;
    }

    /// 返回 having 标志。
    #[must_use]
    pub const fn is_having(&self) -> bool {
        self.having
    }

    /// 设置 having 标志。
    pub fn set_having(&mut self, value: bool) {
        self.having = value;
    }

    /// 返回 join 标志。
    #[must_use]
    pub const fn is_join(&self) -> bool {
        self.join
    }

    /// 设置 join 标志。
    pub fn set_join(&mut self, value: bool) {
        self.join = value;
    }

    /// 返回 primaryKey 标志。
    #[must_use]
    pub const fn is_primary_key(&self) -> bool {
        self.primary_key
    }

    /// 设置 primaryKey 标志。
    pub fn set_primary_key(&mut self, value: bool) {
        self.primary_key = value;
    }

    /// 返回 unique 标志。
    #[must_use]
    pub const fn is_unique(&self) -> bool {
        self.unique
    }

    /// 设置 unique 标志。
    pub fn set_unique(&mut self, value: bool) {
        self.unique = value;
    }

    /// 返回 update 标志。
    #[must_use]
    pub const fn is_update(&self) -> bool {
        self.update
    }

    /// 设置 update 标志。
    pub fn set_update(&mut self, value: bool) {
        self.update = value;
    }

    /// 返回 dataType。
    #[must_use]
    pub fn data_type(&self) -> Option<&str> {
        self.data_type.as_deref()
    }

    /// 设置 dataType。
    pub fn set_data_type(&mut self, value: Option<String>) {
        self.data_type = value;
    }

    /// 返回 attributes。
    #[must_use]
    pub const fn attributes(&self) -> &HashMap<String, Value> {
        &self.attributes
    }

    /// 替换 attributes。
    pub fn set_attributes(&mut self, value: HashMap<String, Value>) {
        self.attributes = value;
    }
}

impl PartialEq for TableStatColumn {
    fn eq(&self, other: &Self) -> bool {
        self.hash_code_64 == other.hash_code_64
    }
}

impl Eq for TableStatColumn {}

impl Hash for TableStatColumn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let java_hash = (self.hash_code_64 ^ (self.hash_code_64 >> 32)) as u32;
        state.write_u32(java_hash);
    }
}

impl Display for TableStatColumn {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(table) = &self.table {
            write!(
                formatter,
                "{}.{}",
                normalize_identifier(table),
                normalize_identifier(&self.name)
            )
        } else {
            formatter.write_str(&normalize_identifier(&self.name))
        }
    }
}

/// SQL 表操作位标记。
///
/// 对应 Java: `TableStat.Mode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableStatMode {
    Insert = 1,
    Update = 2,
    Delete = 4,
    Select = 8,
    Merge = 16,
    Truncate = 32,
    Alter = 64,
    Drop = 128,
    DropIndex = 256,
    CreateIndex = 512,
    Replace = 1024,
    Desc = 2048,
}

impl TableStatMode {
    /// 返回 Java mark。
    #[must_use]
    pub const fn mark(self) -> i32 {
        self as i32
    }
}

fn normalize_identifier(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() > 2
        && matches!(
            (chars[0], chars[chars.len() - 1]),
            ('[', ']') | ('"', '"') | ('`', '`') | ('\'', '\'')
        )
    {
        return chars[1..chars.len() - 1]
            .iter()
            .collect::<String>()
            .trim()
            .replace("`.`", ".");
    }
    name.to_owned()
}

fn normalized_hash(name: &str) -> u64 {
    let normalized = normalize_identifier(name);
    fnv_append(FNV_BASIC, &normalized)
}

fn qualified_hash(table: Option<&str>, name: &str) -> u64 {
    let mut hash = FNV_BASIC;
    if let Some(table) = table {
        for part in table.split('.') {
            hash = fnv_append(hash, &normalize_identifier(part));
            hash ^= u64::from('.');
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    fnv_append(hash, &normalize_identifier(name))
}

fn fnv_append(mut hash: u64, value: &str) -> u64 {
    for mut unit in value.encode_utf16() {
        if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
            unit += 32;
        }
        hash ^= u64::from(unit);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn java_value(value: &Value, quote_string: bool) -> String {
    match value {
        Value::String(value) if quote_string => {
            serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
        }
        Value::String(value) => value.clone(),
        Value::Null => "null".to_owned(),
        value => value.to_string(),
    }
}
