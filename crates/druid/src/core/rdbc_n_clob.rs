//! RDBC `NClob` platform object.
//!
//! Corresponds to Java: `java.sql.NClob`. `NClob` inherits all `Clob` resource operations and
//! identifies content encoded with an SQL national character set.

use super::{PhysicalClob, RdbcClob};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// Physical `NClob` marker SPI.
///
/// Corresponds to Java: `java.sql.NClob extends Clob`.
pub trait PhysicalNClob: PhysicalClob {}

/// Public RDBC `NClob` handle.
///
/// `Deref` exposes all `RdbcClob` operations while retaining a distinct `NClob` identity.
#[derive(Clone)]
pub struct RdbcNClob {
    clob: RdbcClob,
    physical: Arc<dyn PhysicalNClob>,
}

impl RdbcNClob {
    /// Wraps a physical `NClob` adapter.
    pub fn new(physical: Arc<dyn PhysicalNClob>) -> Self {
        let clob_physical: Arc<dyn PhysicalClob> = physical.clone();
        Self {
            clob: RdbcClob::new(clob_physical),
            physical,
        }
    }

    /// Returns the physical `NClob` SPI.
    pub fn physical_n_clob(&self) -> &dyn PhysicalNClob {
        self.physical.as_ref()
    }

    /// Returns the inherited `Clob` handle.
    pub fn as_clob(&self) -> &RdbcClob {
        &self.clob
    }
}

impl Deref for RdbcNClob {
    type Target = RdbcClob;

    fn deref(&self) -> &Self::Target {
        &self.clob
    }
}

impl fmt::Debug for RdbcNClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcNClob")
            .field("clob", &self.clob)
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for RdbcNClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcNClob {}
