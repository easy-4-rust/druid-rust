use super::*;

impl SqlxConnectionAdapter {
    pub(crate) fn decode_mysql_row(row: MySqlRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }
            let type_name = column.type_info().name().to_ascii_uppercase();
            let value = match type_name.as_str() {
                "BOOLEAN" | "BOOL" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "TINYINT" => {
                    let value: i8 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "SMALLINT" => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "MEDIUMINT" | "INT" | "INTEGER" => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "BIGINT" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "FLOAT" => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                "DOUBLE" | "REAL" => Value::Float(row.try_get(index).map_err(Self::driver_error)?),
                "DECIMAL" | "NUMERIC" | "NEWDECIMAL" => {
                    Value::Decimal(row.try_get(index).map_err(Self::driver_error)?)
                }
                "DATE" => Value::Date(row.try_get(index).map_err(Self::driver_error)?),
                "TIME" => Value::Time(row.try_get(index).map_err(Self::driver_error)?),
                "DATETIME" => Value::Timestamp(row.try_get(index).map_err(Self::driver_error)?),
                "TIMESTAMP" => {
                    // 仅在 MySQL 列类型已确认为 TIMESTAMP 后使用其日期时间解码器。
                    // SQLx 的 NaiveDateTime 类型检查只接受 DATETIME，但两者共享该解码器；
                    // 保留会话返回的年月日时分秒，不额外转换时区或绕过其他列的类型检查。
                    Value::Timestamp(
                        row.try_get_unchecked::<NaiveDateTime, _>(index)
                            .map_err(Self::driver_error)?,
                    )
                }
                name if name.contains("BLOB") || name.contains("BINARY") => {
                    Value::Bytes(row.try_get(index).map_err(Self::driver_error)?)
                }
                name if name.contains("CHAR")
                    || name.contains("TEXT")
                    || std::matches!(name, "ENUM" | "SET" | "JSON") =>
                {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "MySQL type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    pub(crate) fn decode_postgresql_row(row: PgRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }
            let type_name = column.type_info().name().to_ascii_uppercase();
            let value = match type_name.as_str() {
                "BOOL" | "BOOLEAN" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "INT2" | "SMALLINT" => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "INT4" | "INTEGER" => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "INT8" | "BIGINT" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "FLOAT4" | "REAL" => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                "FLOAT8" | "DOUBLE PRECISION" => {
                    Value::Float(row.try_get(index).map_err(Self::driver_error)?)
                }
                "NUMERIC" | "DECIMAL" => {
                    Value::Decimal(row.try_get(index).map_err(Self::driver_error)?)
                }
                "DATE" => Value::Date(row.try_get(index).map_err(Self::driver_error)?),
                "TIME" => Value::Time(row.try_get(index).map_err(Self::driver_error)?),
                "TIMESTAMP" => Value::Timestamp(row.try_get(index).map_err(Self::driver_error)?),
                "BYTEA" => Value::Bytes(row.try_get(index).map_err(Self::driver_error)?),
                "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" => {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "PostgreSQL type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    pub(crate) fn decode_sqlite_row(row: SqliteRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }

            // SQLite 的表达式列（如 COUNT(*)）没有声明类型，SQLx 会把
            // column type 标为 NULL，但运行时 value type 仍是 INTEGER/REAL 等。
            // 普通 BOOLEAN 列仍优先使用声明类型，保留 Druid 的布尔值语义。
            let runtime_type = raw.type_info();
            let type_name = match column.type_info().name() {
                "NULL" => runtime_type.name(),
                declared => declared,
            };
            let value = match type_name {
                "BOOLEAN" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "INTEGER" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "REAL" => Value::Float(row.try_get(index).map_err(Self::driver_error)?),
                "NUMERIC" | "DECIMAL" => {
                    let value = match runtime_type.name() {
                        "INTEGER" => BigDecimal::from(
                            row.try_get::<i64, _>(index).map_err(Self::driver_error)?,
                        ),
                        "REAL" => BigDecimal::from_f64(
                            row.try_get::<f64, _>(index).map_err(Self::driver_error)?,
                        )
                        .ok_or_else(|| {
                            RdbcError::DriverError(
                                "SQLite REAL cannot be represented as BigDecimal".to_string(),
                            )
                        })?,
                        _ => row
                            .try_get::<String, _>(index)
                            .map_err(Self::driver_error)?
                            .parse::<BigDecimal>()
                            .map_err(|error| RdbcError::DriverError(error.to_string()))?,
                    };
                    Value::Decimal(value)
                }
                "DATE" => Value::Date(
                    row.try_get::<NaiveDate, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "TIME" => Value::Time(
                    row.try_get::<NaiveTime, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "DATETIME" | "TIMESTAMP" => Value::Timestamp(
                    row.try_get::<NaiveDateTime, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "TEXT" => Value::String(row.try_get(index).map_err(Self::driver_error)?),
                "BLOB" => Value::Bytes(row.try_get(index).map_err(Self::driver_error)?),
                "NULL" => Value::Null,
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "SQLite type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    pub(crate) fn column_descriptor(
        name: &str,
        type_name: &str,
        nullable: Option<bool>,
    ) -> ColumnDescriptor {
        let uppercase = type_name.to_ascii_uppercase();
        let (column_type, signed, case_sensitive) = match uppercase.as_str() {
            "BOOL" | "BOOLEAN" => (ResultSetColumnType::Jdbc(Types::BOOLEAN), false, false),
            "TINYINT" => (ResultSetColumnType::Jdbc(Types::TINYINT), true, false),
            "SMALLINT" | "INT2" => (ResultSetColumnType::Jdbc(Types::SMALLINT), true, false),
            "INTEGER" | "INT" | "INT4" | "MEDIUMINT" => {
                (ResultSetColumnType::Jdbc(Types::INTEGER), true, false)
            }
            "BIGINT" | "INT8" => (ResultSetColumnType::Jdbc(Types::BIGINT), true, false),
            "REAL" | "FLOAT4" => (ResultSetColumnType::Jdbc(Types::REAL), true, false),
            "FLOAT" => (ResultSetColumnType::Jdbc(Types::FLOAT), true, false),
            "DOUBLE" | "FLOAT8" | "DOUBLE PRECISION" => {
                (ResultSetColumnType::Jdbc(Types::DOUBLE), true, false)
            }
            "NUMERIC" | "DECIMAL" | "NEWDECIMAL" => {
                (ResultSetColumnType::Jdbc(Types::DECIMAL), true, false)
            }
            "DATE" => (ResultSetColumnType::Jdbc(Types::DATE), false, false),
            "TIME" => (ResultSetColumnType::Jdbc(Types::TIME), false, false),
            "DATETIME" | "TIMESTAMP" => (ResultSetColumnType::Jdbc(Types::TIMESTAMP), false, false),
            value if value.contains("BLOB") || value.contains("BINARY") || value == "BYTEA" => {
                (ResultSetColumnType::Jdbc(Types::VARBINARY), false, false)
            }
            value
                if value.contains("CHAR")
                    || value.contains("TEXT")
                    || std::matches!(value, "NAME" | "ENUM" | "SET" | "JSON") =>
            {
                (ResultSetColumnType::Jdbc(Types::VARCHAR), false, true)
            }
            "NULL" => (ResultSetColumnType::Jdbc(Types::NULL), false, false),
            other => (ResultSetColumnType::Named(other.to_owned()), false, true),
        };
        let nullability = match nullable {
            Some(true) => ColumnNullability::Nullable,
            Some(false) => ColumnNullability::NoNulls,
            None => ColumnNullability::Unknown,
        };
        ColumnDescriptor::new(name, type_name, nullability)
            .with_driver_type(name, column_type)
            .with_flags(false, case_sensitive, true, false, signed)
    }

    pub(crate) async fn describe_columns_core(
        &mut self,
        sql: &str,
    ) -> Result<Vec<ColumnDescriptor>, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
        }
    }

    pub(crate) async fn fetch_rows_with_labels(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<(Vec<Row>, Vec<ColumnDescriptor>), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_any_prepared_values(&statement, params)?
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_any_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_mysql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_mysql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_postgresql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_postgresql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_sqlite_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_sqlite_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
        }
    }

    /// 在原物理连接上执行按需读取；驱动自有缓冲与 wrapper 请求容量分别受各自预算约束。
    pub(crate) async fn serve_cursor_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        startup: &mut Option<
            tokio::sync::oneshot::Sender<Result<Vec<ColumnDescriptor>, RdbcError>>,
        >,
        requests: &mut tokio::sync::mpsc::Receiver<
            super::super::sqlx_cursor_request::SqlxCursorRequest,
        >,
        stop: &mut tokio::sync::watch::Receiver<bool>,
        parent_stop: &mut tokio::sync::watch::Receiver<bool>,
        statement_stop: &mut tokio::sync::watch::Receiver<bool>,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_any_prepared_values(&statement, params)?.fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_any_row)
                });
                super::super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_mysql_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_mysql_row)
                });
                super::super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows =
                    Self::bind_postgresql_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_postgresql_row)
                });
                super::super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_sqlite_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_sqlite_row)
                });
                super::super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
        }
    }

    pub(crate) async fn standard_parameter_count_core(
        &mut self,
        sql: &str,
    ) -> Result<usize, RdbcError> {
        fn count<'query, S: Statement<'query>>(statement: &S) -> Result<usize, RdbcError> {
            match statement.parameters() {
                Some(sqlx::Either::Left(types)) => Ok(types.len()),
                Some(sqlx::Either::Right(count)) => Ok(count),
                None => Err(RdbcError::UnsupportedOperation {
                    operation: "sqlx_parameter_metadata_unavailable",
                }),
            }
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::MySql(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
        }
    }

    pub(crate) async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<SqlxExecResult, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let result = Self::bind_any_values(sql, params)?
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: result.last_insert_id(),
                    row_count: None,
                })
            }
            SqlxConnectionBackend::MySql(connection) => {
                let result = Self::bind_mysql_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: (result.last_insert_id() != 0)
                        .then(|| i64::try_from(result.last_insert_id()).unwrap_or(i64::MAX)),
                    row_count: None,
                })
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let result = Self::bind_postgresql_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: None,
                    row_count: None,
                })
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let result = Self::bind_sqlite_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                // 驱动在每次操作后都报告连接级 rowid；只有本次非零 INSERT 才能发布它。
                let inserts_rows = matches!(
                    sqlparser::parser::Parser::parse_sql(
                        &sqlparser::dialect::SQLiteDialect {},
                        sql
                    )
                    .as_deref(),
                    Ok([sqlparser::ast::Statement::Insert(_)])
                );
                let last_insert_id =
                    (inserts_rows && result.rows_affected() > 0 && result.last_insert_rowid() != 0)
                        .then_some(result.last_insert_rowid());
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id,
                    row_count: None,
                })
            }
        }
    }
}
