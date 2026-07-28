//! 对应 Java 类：com.alibaba.druid.wall.WallProvider + WallVisitor
//!
//! SQL 防火墙，基于 sqlparser-rs AST 检查 SQL 安全性。

use super::wall_config::WallConfig;
use super::wall_violation::WallViolation;
use sqlparser::ast::{
    FromTable, ObjectName, ObjectType, Query, Select, SetExpr, Statement, TableFactor,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::sync::Arc;

/// SQL 防火墙。
pub struct Wall {
    config: Arc<WallConfig>,
}

impl Wall {
    pub fn new(config: WallConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub fn config(&self) -> &WallConfig {
        &self.config
    }

    /// 检查 SQL 是否合规。
    pub fn check(&self, sql: &str) -> Result<(), Vec<WallViolation>> {
        let dialect = GenericDialect {};
        let ast = Parser::parse_sql(&dialect, sql)
            .map_err(|e| vec![WallViolation::SyntaxError(e.to_string())])?;
        let mut violations = Vec::new();
        for stmt in &ast {
            self.check_statement(stmt, &mut violations);
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    fn check_statement(&self, stmt: &Statement, v: &mut Vec<WallViolation>) {
        match stmt {
            Statement::Query(query) => {
                self.check_query(query, v);
            }
            Statement::Delete(delete) => {
                if !self.config.delete_allow {
                    v.push(WallViolation::SyntaxError("DELETE not allowed".into()));
                }
                if delete.selection.is_none() && self.config.delete_must_have_where {
                    v.push(WallViolation::DeleteWithoutWhere);
                }
                self.check_from_table(&delete.from, v);
            }
            Statement::Update {
                table, selection, ..
            } => {
                if !self.config.update_allow {
                    v.push(WallViolation::SyntaxError("UPDATE not allowed".into()));
                }
                if selection.is_none() && self.config.update_must_have_where {
                    v.push(WallViolation::UpdateWithoutWhere);
                }
                self.check_table_factor(&table.relation, v);
            }
            Statement::Insert(insert) => {
                if !self.config.insert_allow {
                    v.push(WallViolation::SyntaxError("INSERT not allowed".into()));
                }
                self.check_object_name(&insert.table_name, v);
            }
            Statement::Drop {
                object_type, names, ..
            } => {
                if *object_type == ObjectType::Table {
                    if !self.config.drop_table_allow {
                        for name in names {
                            v.push(WallViolation::DropTableNotAllowed(name.to_string()));
                        }
                    }
                }
            }
            Statement::Truncate { table_names, .. } => {
                if !self.config.truncate_allow {
                    v.push(WallViolation::TruncateNotAllowed);
                }
                for target in table_names {
                    self.check_object_name(&target.name, v);
                }
            }
            _ => {}
        }
    }

    fn check_query(&self, query: &Query, v: &mut Vec<WallViolation>) {
        match &*query.body {
            SetExpr::Select(select) => {
                self.check_select(select, v);
            }
            SetExpr::Query(subquery) => {
                self.check_query(subquery, v);
            }
            SetExpr::SetOperation { left, right, .. } => {
                if let SetExpr::Select(l) = &**left {
                    self.check_select(l, v);
                }
                if let SetExpr::Select(r) = &**right {
                    self.check_select(r, v);
                }
            }
            _ => {}
        }
    }

    fn check_select(&self, select: &Select, v: &mut Vec<WallViolation>) {
        // 检查 FROM 子句中的表
        for table_with_joins in &select.from {
            self.check_table_factor(&table_with_joins.relation, v);
        }
    }

    fn check_from_table(&self, from: &FromTable, v: &mut Vec<WallViolation>) {
        let tables = match from {
            FromTable::WithFromKeyword(t) | FromTable::WithoutKeyword(t) => t,
        };
        for twj in tables {
            self.check_table_factor(&twj.relation, v);
        }
    }

    fn check_table_factor(&self, factor: &TableFactor, v: &mut Vec<WallViolation>) {
        match factor {
            TableFactor::Table { name, .. } => {
                self.check_object_name(name, v);
            }
            TableFactor::Derived { subquery, .. } => {
                self.check_query(subquery, v);
            }
            _ => {}
        }
    }

    fn check_object_name(&self, name: &ObjectName, v: &mut Vec<WallViolation>) {
        let name_str = name.to_string().to_lowercase();
        for deny in &self.config.deny_tables {
            if name_str.contains(&deny.to_lowercase()) {
                v.push(WallViolation::DeniedTable(name_str.clone()));
            }
        }
    }
}
