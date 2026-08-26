use super::SqlxDatabaseMetaData;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, DatabaseMetaDataArgument, DatabaseMetaDataOperation,
    DatabaseMetaDataRequest, TablesQuery,
};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{Row, Types, Value};

impl SqlxDatabaseMetaData {
    pub(super) async fn metadata_result_set_core(
        &self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.require_sqlite(request.operation())?;
        match request.operation() {
            DatabaseMetaDataOperation::GetTables => {
                self.actor
                    .tables(TablesQuery {
                        catalog: optional_string(&request, 0)?,
                        schema_pattern: optional_string(&request, 1)?,
                        table_name_pattern: optional_string(&request, 2)?,
                        table_types: optional_strings(&request, 3)?,
                    })
                    .await
            }
            DatabaseMetaDataOperation::GetSchemas
            | DatabaseMetaDataOperation::GetSchemasWithPattern => {
                Ok(self.empty_result_set(&["TABLE_SCHEM", "TABLE_CATALOG"]))
            }
            DatabaseMetaDataOperation::GetCatalogs => Ok(self.empty_result_set(&["TABLE_CAT"])),
            DatabaseMetaDataOperation::GetTableTypes => Ok(self.result_set(
                ["GLOBAL TEMPORARY", "SYSTEM TABLE", "TABLE", "VIEW"]
                    .into_iter()
                    .map(|kind| Row::new(vec![Value::String(kind.to_owned())]))
                    .collect(),
                &["TABLE_TYPE"],
            )),
            DatabaseMetaDataOperation::GetColumns => self.columns(&request).await,
            DatabaseMetaDataOperation::GetPrimaryKeys => self.primary_keys(&request).await,
            DatabaseMetaDataOperation::GetImportedKeys => self.imported_keys(&request).await,
            DatabaseMetaDataOperation::GetExportedKeys => self.exported_keys(&request).await,
            DatabaseMetaDataOperation::GetCrossReference => self.cross_reference(&request).await,
            DatabaseMetaDataOperation::GetIndexInfo => self.index_info(&request).await,
            DatabaseMetaDataOperation::GetTypeInfo => Ok(self.type_info()),
            DatabaseMetaDataOperation::GetProcedures => Ok(self.empty_result_set(&[
                "PROCEDURE_CAT",
                "PROCEDURE_SCHEM",
                "PROCEDURE_NAME",
                "UNDEF1",
                "UNDEF2",
                "UNDEF3",
                "REMARKS",
                "PROCEDURE_TYPE",
            ])),
            DatabaseMetaDataOperation::GetProcedureColumns => Ok(self.empty_result_set(&[
                "PROCEDURE_CAT",
                "PROCEDURE_SCHEM",
                "PROCEDURE_NAME",
                "COLUMN_NAME",
                "COLUMN_TYPE",
                "DATA_TYPE",
                "TYPE_NAME",
                "PRECISION",
                "LENGTH",
                "SCALE",
                "RADIX",
                "NULLABLE",
                "REMARKS",
            ])),
            DatabaseMetaDataOperation::GetColumnPrivileges => Ok(self.empty_result_set(&[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "COLUMN_NAME",
                "GRANTOR",
                "GRANTEE",
                "PRIVILEGE",
                "IS_GRANTABLE",
            ])),
            DatabaseMetaDataOperation::GetTablePrivileges => Ok(self.empty_result_set(&[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "GRANTOR",
                "GRANTEE",
                "PRIVILEGE",
                "IS_GRANTABLE",
            ])),
            DatabaseMetaDataOperation::GetBestRowIdentifier
            | DatabaseMetaDataOperation::GetVersionColumns => Ok(self.empty_result_set(&[
                "SCOPE",
                "COLUMN_NAME",
                "DATA_TYPE",
                "TYPE_NAME",
                "COLUMN_SIZE",
                "BUFFER_LENGTH",
                "DECIMAL_DIGITS",
                "PSEUDO_COLUMN",
            ])),
            DatabaseMetaDataOperation::GetUdTs => Ok(self.empty_result_set(&[
                "TYPE_CAT",
                "TYPE_SCHEM",
                "TYPE_NAME",
                "CLASS_NAME",
                "DATA_TYPE",
                "REMARKS",
                "BASE_TYPE",
            ])),
            DatabaseMetaDataOperation::GetSuperTypes => Ok(self.empty_result_set(&[
                "TYPE_CAT",
                "TYPE_SCHEM",
                "TYPE_NAME",
                "SUPERTYPE_CAT",
                "SUPERTYPE_SCHEM",
                "SUPERTYPE_NAME",
            ])),
            DatabaseMetaDataOperation::GetSuperTables => Ok(self.empty_result_set(&[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "SUPERTABLE_NAME",
            ])),
            operation => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    fn require_sqlite(&self, operation: DatabaseMetaDataOperation) -> Result<(), RdbcError> {
        if self.actor.backend_key() == "sqlite" {
            Ok(())
        } else {
            Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            })
        }
    }

    fn result_set(&self, rows: Vec<Row>, labels: &[&str]) -> Box<dyn ResultSet> {
        let columns = (0..labels.len())
            .map(|offset| {
                let type_name = rows
                    .iter()
                    .filter_map(|row| row.get(offset))
                    .find(|value| !std::matches!(value, Value::Null))
                    .map(value_type_name)
                    .unwrap_or("NULL");
                let nullable = rows.iter().any(|row| {
                    row.get(offset)
                        .is_none_or(|value| std::matches!(value, Value::Null))
                });
                ColumnDescriptor::new(
                    labels[offset],
                    type_name,
                    if nullable {
                        ColumnNullability::Nullable
                    } else {
                        ColumnNullability::NoNulls
                    },
                )
            })
            .collect();
        self.actor.result_set_core(rows, columns)
    }

    fn empty_result_set(&self, labels: &[&str]) -> Box<dyn ResultSet> {
        self.result_set(Vec::new(), labels)
    }

    async fn table_names(&self, pattern: Option<String>) -> Result<Vec<String>, RdbcError> {
        let rows = self
            .actor
            .fetch_rows(
                "select name from sqlite_schema where type in ('table','view') and name like ? order by name",
                vec![Value::String(pattern.unwrap_or_else(|| "%".to_owned()))],
            )
            .await?;
        rows.iter().map(|row| row_string(row, 0)).collect()
    }

    async fn columns(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let table_pattern = optional_string(request, 2)?;
        let column_pattern = optional_string(request, 3)?.unwrap_or_else(|| "%".to_owned());
        let mut output = Vec::new();
        for table in self.table_names(table_pattern).await? {
            let pragma = format!("pragma table_xinfo({})", sqlite_string_literal(&table));
            let rows = self.actor.fetch_rows(&pragma, Vec::new()).await?;
            for row in &rows {
                let name = row_string(row, 1)?;
                if !sqlite_like(&name, &column_pattern) {
                    continue;
                }
                let declared_type = row_optional_string(row, 2)?;
                let not_null = row_integer(row, 3)?;
                let default_value = row.get(4).cloned().unwrap_or(Value::Null);
                let primary_key = row_integer(row, 5)?;
                let hidden = row_integer(row, 6)?;
                let (data_type, type_name) = declared_type
                    .as_deref()
                    .map(sqlite_type)
                    .unwrap_or((Types::VARCHAR, "TEXT".to_owned()));
                output.push(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table.clone()),
                    Value::String(name),
                    Value::Int(i64::from(data_type)),
                    Value::String(type_name),
                    Value::Int(2_000_000_000),
                    Value::Int(2_000_000_000),
                    Value::Int(0),
                    Value::Int(10),
                    Value::Int(i64::from(not_null == 0)),
                    Value::Null,
                    default_value,
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(2_000_000_000),
                    Value::Int(row_integer(row, 0)? + 1),
                    Value::String(if not_null == 0 { "YES" } else { "NO" }.to_owned()),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::String(if primary_key == 1 { "YES" } else { "NO" }.to_owned()),
                    Value::String(
                        if hidden == 2 || hidden == 3 {
                            "YES"
                        } else {
                            "NO"
                        }
                        .to_owned(),
                    ),
                ]));
            }
        }
        Ok(self.result_set(output, COLUMN_LABELS))
    }

    async fn primary_keys(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let table = required_table(request, 2)?;
        let pragma = format!("pragma table_info({})", sqlite_string_literal(&table));
        let rows = self.actor.fetch_rows(&pragma, Vec::new()).await?;
        let mut keys = rows
            .iter()
            .map(|row| Ok((row_integer(row, 5)?, row_string(row, 1)?)))
            .filter_map(|key: Result<(i64, String), RdbcError>| match key {
                Ok((0, _)) => None,
                key => Some(key),
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        keys.sort();
        let output = keys
            .into_iter()
            .map(|(sequence, name)| {
                Ok(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table.clone()),
                    Value::String(name),
                    Value::Int(sequence),
                    Value::Null,
                ]))
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        Ok(self.result_set(
            output,
            &[
                "TABLE_CAT",
                "TABLE_SCHEM",
                "TABLE_NAME",
                "COLUMN_NAME",
                "KEY_SEQ",
                "PK_NAME",
            ],
        ))
    }

    async fn imported_keys(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let table = required_table(request, 2)?;
        self.foreign_keys(&table, None).await
    }

    async fn exported_keys(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let parent = required_table(request, 2)?;
        let mut rows = Vec::new();
        for foreign in self.table_names(None).await? {
            rows.extend(self.foreign_key_rows(&foreign, Some(&parent)).await?);
        }
        Ok(self.result_set(rows, FOREIGN_KEY_LABELS))
    }

    async fn cross_reference(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let parent = optional_string(request, 2)?;
        let foreign = optional_string(request, 5)?;
        match (parent, foreign) {
            (Some(parent), Some(foreign)) => self.foreign_keys(&foreign, Some(&parent)).await,
            (Some(parent), None) => {
                let mut rows = Vec::new();
                for foreign in self.table_names(None).await? {
                    rows.extend(self.foreign_key_rows(&foreign, Some(&parent)).await?);
                }
                Ok(self.result_set(rows, FOREIGN_KEY_LABELS))
            }
            (None, Some(foreign)) => self.foreign_keys(&foreign, None).await,
            (None, None) => Ok(self.empty_result_set(FOREIGN_KEY_LABELS)),
        }
    }

    async fn foreign_keys(
        &self,
        foreign: &str,
        parent: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Ok(self.result_set(
            self.foreign_key_rows(foreign, parent).await?,
            FOREIGN_KEY_LABELS,
        ))
    }

    async fn foreign_key_rows(
        &self,
        foreign: &str,
        parent: Option<&str>,
    ) -> Result<Vec<Row>, RdbcError> {
        let pragma = format!(
            "pragma foreign_key_list({})",
            sqlite_string_literal(foreign)
        );
        let rows = self.actor.fetch_rows(&pragma, Vec::new()).await?;
        rows.iter()
            .filter(|row| {
                parent.is_none_or(|parent| {
                    row_string(row, 2).is_ok_and(|value| value.eq_ignore_ascii_case(parent))
                })
            })
            .map(|row| {
                Ok(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(row_string(row, 2)?),
                    row.get(4).cloned().unwrap_or(Value::Null),
                    Value::Null,
                    Value::Null,
                    Value::String(foreign.to_owned()),
                    Value::String(row_string(row, 3)?),
                    Value::Int(row_integer(row, 1)? + 1),
                    Value::Int(i64::from(imported_key_rule(&row_string(row, 5)?))),
                    Value::Int(i64::from(imported_key_rule(&row_string(row, 6)?))),
                    Value::Null,
                    Value::Null,
                    Value::Int(5),
                ]))
            })
            .collect()
    }

    async fn index_info(
        &self,
        request: &DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let table = required_table(request, 2)?;
        let unique_only = boolean_argument(request, 3)?;
        let index_list_pragma = format!(
            "pragma index_list({})",
            sqlite_string_literal(table.as_str())
        );
        let indexes = self
            .actor
            .fetch_rows(&index_list_pragma, Vec::new())
            .await?;
        let mut output = Vec::new();
        for index in indexes {
            let name = row_string(&index, 1)?;
            let unique = row_integer(&index, 2)? != 0;
            if unique_only && !unique {
                continue;
            }
            let index_info_pragma = format!(
                "pragma index_info({})",
                sqlite_string_literal(name.as_str())
            );
            let columns = self
                .actor
                .fetch_rows(&index_info_pragma, Vec::new())
                .await?;
            for column in columns {
                output.push(Row::new(vec![
                    Value::Null,
                    Value::Null,
                    Value::String(table.clone()),
                    Value::Bool(!unique),
                    Value::Null,
                    Value::String(name.clone()),
                    Value::Int(3),
                    Value::Int(row_integer(&column, 0)? + 1),
                    column.get(2).cloned().unwrap_or(Value::Null),
                    Value::Null,
                    Value::Int(0),
                    Value::Int(0),
                    Value::Null,
                ]));
            }
        }
        Ok(self.result_set(output, INDEX_LABELS))
    }

    fn type_info(&self) -> Box<dyn ResultSet> {
        let row = |name: &str, code: i32, case_sensitive: bool| {
            Row::new(vec![
                Value::String(name.to_owned()),
                Value::Int(i64::from(code)),
                Value::Int(0),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Int(1),
                Value::Bool(case_sensitive),
                Value::Int(3),
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(false),
                Value::Null,
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(10),
            ])
        };
        self.result_set(
            vec![
                row("NULL", Types::NULL, false),
                row("INTEGER", Types::INTEGER, false),
                row("REAL", Types::REAL, false),
                row("TEXT", Types::VARCHAR, true),
                row("BLOB", Types::BLOB, false),
            ],
            TYPE_INFO_LABELS,
        )
    }
}

fn optional_string(
    request: &DatabaseMetaDataRequest,
    index: usize,
) -> Result<Option<String>, RdbcError> {
    match request.arguments().get(index) {
        Some(DatabaseMetaDataArgument::OptionalString(value)) => Ok(value.clone()),
        _ => Err(RdbcError::InvalidArgument(format!(
            "metadata argument {index} is not OptionalString"
        ))),
    }
}

fn optional_strings(
    request: &DatabaseMetaDataRequest,
    index: usize,
) -> Result<Option<Vec<String>>, RdbcError> {
    match request.arguments().get(index) {
        Some(DatabaseMetaDataArgument::Strings(value)) => Ok(value.clone()),
        _ => Err(RdbcError::InvalidArgument(format!(
            "metadata argument {index} is not Strings"
        ))),
    }
}

fn boolean_argument(request: &DatabaseMetaDataRequest, index: usize) -> Result<bool, RdbcError> {
    match request.arguments().get(index) {
        Some(DatabaseMetaDataArgument::Boolean(value)) => Ok(*value),
        _ => Err(RdbcError::InvalidArgument(format!(
            "metadata argument {index} is not Boolean"
        ))),
    }
}

fn required_table(request: &DatabaseMetaDataRequest, index: usize) -> Result<String, RdbcError> {
    optional_string(request, index)?
        .ok_or_else(|| RdbcError::InvalidArgument("table name is SQL NULL".to_owned()))
}

fn row_string(row: &Row, index: usize) -> Result<String, RdbcError> {
    match row.get(index) {
        Some(Value::String(value)) => Ok(value.clone()),
        actual => Err(RdbcError::DriverError(format!(
            "metadata column {index} expected string, got {actual:?}"
        ))),
    }
}

fn row_optional_string(row: &Row, index: usize) -> Result<Option<String>, RdbcError> {
    match row.get(index) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        actual => Err(RdbcError::DriverError(format!(
            "metadata column {index} expected nullable string, got {actual:?}"
        ))),
    }
}

fn row_integer(row: &Row, index: usize) -> Result<i64, RdbcError> {
    match row.get(index) {
        Some(Value::Int(value)) => Ok(*value),
        actual => Err(RdbcError::DriverError(format!(
            "metadata column {index} expected integer, got {actual:?}"
        ))),
    }
}

fn sqlite_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sqlite_like(value: &str, pattern: &str) -> bool {
    #[derive(Clone, Copy)]
    enum Token {
        AnySequence,
        AnyCharacter,
        Literal(char),
    }

    let mut tokens = Vec::new();
    let mut pattern_chars = pattern.chars();
    while let Some(character) = pattern_chars.next() {
        match character {
            '%' => tokens.push(Token::AnySequence),
            '_' => tokens.push(Token::AnyCharacter),
            '\\' => tokens.push(Token::Literal(pattern_chars.next().unwrap_or('\\'))),
            literal => tokens.push(Token::Literal(literal)),
        }
    }
    let value = value.chars().collect::<Vec<_>>();
    let mut matches = vec![vec![false; value.len() + 1]; tokens.len() + 1];
    matches[0][0] = true;
    for token_index in 1..=tokens.len() {
        for value_index in 0..=value.len() {
            matches[token_index][value_index] = match tokens[token_index - 1] {
                Token::AnySequence => {
                    matches[token_index - 1][value_index]
                        || (value_index > 0 && matches[token_index][value_index - 1])
                }
                Token::AnyCharacter => value_index > 0 && matches[token_index - 1][value_index - 1],
                Token::Literal(expected) => {
                    value_index > 0
                        && matches[token_index - 1][value_index - 1]
                        && expected.eq_ignore_ascii_case(&value[value_index - 1])
                }
            };
        }
    }
    matches[tokens.len()][value.len()]
}

fn sqlite_type(declared: &str) -> (i32, String) {
    let uppercase = declared.to_ascii_uppercase();
    let code = if uppercase.contains("INT") {
        Types::INTEGER
    } else if uppercase.contains("REAL") || uppercase.contains("FLOA") || uppercase.contains("DOUB")
    {
        Types::REAL
    } else if uppercase.contains("BLOB") {
        Types::BLOB
    } else {
        Types::VARCHAR
    };
    (
        code,
        uppercase
            .split('(')
            .next()
            .unwrap_or("TEXT")
            .trim()
            .to_owned(),
    )
}

fn imported_key_rule(rule: &str) -> i32 {
    match rule {
        "CASCADE" => 0,
        "RESTRICT" => 1,
        "SET NULL" => 2,
        "SET DEFAULT" => 4,
        _ => 3,
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NULL",
        Value::Bool(_) => "BOOLEAN",
        Value::Int(_) => "BIGINT",
        Value::Float(_) => "DOUBLE",
        Value::Decimal(_) => "DECIMAL",
        Value::Date(_) => "DATE",
        Value::Time(_) => "TIME",
        Value::Timestamp(_) => "TIMESTAMP",
        Value::String(_) => "VARCHAR",
        Value::Bytes(_) => "VARBINARY",
    }
}

const COLUMN_LABELS: &[&str] = &[
    "TABLE_CAT",
    "TABLE_SCHEM",
    "TABLE_NAME",
    "COLUMN_NAME",
    "DATA_TYPE",
    "TYPE_NAME",
    "COLUMN_SIZE",
    "BUFFER_LENGTH",
    "DECIMAL_DIGITS",
    "NUM_PREC_RADIX",
    "NULLABLE",
    "REMARKS",
    "COLUMN_DEF",
    "SQL_DATA_TYPE",
    "SQL_DATETIME_SUB",
    "CHAR_OCTET_LENGTH",
    "ORDINAL_POSITION",
    "IS_NULLABLE",
    "SCOPE_CATALOG",
    "SCOPE_SCHEMA",
    "SCOPE_TABLE",
    "SOURCE_DATA_TYPE",
    "IS_AUTOINCREMENT",
    "IS_GENERATEDCOLUMN",
];
const FOREIGN_KEY_LABELS: &[&str] = &[
    "PKTABLE_CAT",
    "PKTABLE_SCHEM",
    "PKTABLE_NAME",
    "PKCOLUMN_NAME",
    "FKTABLE_CAT",
    "FKTABLE_SCHEM",
    "FKTABLE_NAME",
    "FKCOLUMN_NAME",
    "KEY_SEQ",
    "UPDATE_RULE",
    "DELETE_RULE",
    "FK_NAME",
    "PK_NAME",
    "DEFERRABILITY",
];
const INDEX_LABELS: &[&str] = &[
    "TABLE_CAT",
    "TABLE_SCHEM",
    "TABLE_NAME",
    "NON_UNIQUE",
    "INDEX_QUALIFIER",
    "INDEX_NAME",
    "TYPE",
    "ORDINAL_POSITION",
    "COLUMN_NAME",
    "ASC_OR_DESC",
    "CARDINALITY",
    "PAGES",
    "FILTER_CONDITION",
];
const TYPE_INFO_LABELS: &[&str] = &[
    "TYPE_NAME",
    "DATA_TYPE",
    "PRECISION",
    "LITERAL_PREFIX",
    "LITERAL_SUFFIX",
    "CREATE_PARAMS",
    "NULLABLE",
    "CASE_SENSITIVE",
    "SEARCHABLE",
    "UNSIGNED_ATTRIBUTE",
    "FIXED_PREC_SCALE",
    "AUTO_INCREMENT",
    "LOCAL_TYPE_NAME",
    "MINIMUM_SCALE",
    "MAXIMUM_SCALE",
    "SQL_DATA_TYPE",
    "SQL_DATETIME_SUB",
    "NUM_PREC_RADIX",
];
