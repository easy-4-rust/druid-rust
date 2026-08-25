//! Wrapper-private owned query result shared by canonical adapter handles.

use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability};
use druid_rdbc::types::Row;

/// Owns materialized rows together with the driver-provided column descriptors.
pub(crate) struct StandardQueryResult {
    pub(crate) rows: Vec<Row>,
    pub(crate) columns: Vec<ColumnDescriptor>,
}

impl StandardQueryResult {
    pub(crate) fn with_labels(rows: Vec<Row>, labels: Vec<String>) -> Self {
        let width = rows.first().map_or(labels.len(), |row| row.values.len());
        let columns = (0..width)
            .map(|index| {
                ColumnDescriptor::new(
                    labels
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| format!("column_{}", index + 1)),
                    "UNKNOWN",
                    ColumnNullability::Unknown,
                )
            })
            .collect();
        Self { rows, columns }
    }

    pub(crate) fn rows_only(rows: Vec<Row>) -> Self {
        Self::with_labels(rows, Vec::new())
    }
}
