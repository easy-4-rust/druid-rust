//! Concrete driver extension registrations for well-known databases.
//!
//! Each `inventory::submit!` registers a `DriverExtensionDescriptor` that
//! Core's `lookup_driver_extension` can resolve by database type.

use druid::core::{
    Db2ExceptionSorter, MySqlExceptionSorter, MySqlValidConnectionChecker,
    OceanBaseOracleExceptionSorter, OceanBaseValidConnectionChecker, OracleExceptionSorter,
    OracleValidConnectionChecker, PgExceptionSorter, PgValidConnectionChecker,
};
use druid::core::{DriverExtensionDescriptor, DriverExtensionFactoryFuture};
use druid_rdbc::driver::ConnectionFactory;
use std::sync::Arc;

use crate::toasty::ToastyConnectionFactory;

/// Initializes the driver extension registry.
///
/// This function exists to ensure the linker retains the `inventory::submit!`
/// statics in this module. Call `druid_wrapper::init_driver_extensions()` or
/// reference this function to guarantee the registrations are available.
pub fn init() {
    // The function body is intentionally empty; its purpose is to create
    // a code reference from lib.rs that prevents the linker from stripping
    // the inventory statics defined in this module.
}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

fn toasty_factory(url: &str) -> DriverExtensionFactoryFuture<'_> {
    Box::pin(async move {
        let factory = ToastyConnectionFactory::new(url).await?;
        Ok(Arc::new(factory) as Arc<dyn ConnectionFactory>)
    })
}

// ---------------------------------------------------------------------------
// MySQL
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "mysql",
        factory: toasty_factory,
        checker: Some(|| Arc::new(MySqlValidConnectionChecker::new())),
        sorter: Some(|| Arc::new(MySqlExceptionSorter)),
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "postgresql",
        factory: toasty_factory,
        checker: Some(|| Arc::new(PgValidConnectionChecker)),
        sorter: Some(|| Arc::new(PgExceptionSorter)),
    }
}

// ---------------------------------------------------------------------------
// Oracle
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "oracle",
        factory: toasty_factory,
        checker: Some(|| Arc::new(OracleValidConnectionChecker::new())),
        sorter: Some(|| Arc::new(OracleExceptionSorter::new())),
    }
}

// ---------------------------------------------------------------------------
// DB2
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "db2",
        factory: toasty_factory,
        checker: None,
        sorter: Some(|| Arc::new(Db2ExceptionSorter)),
    }
}

// ---------------------------------------------------------------------------
// OceanBase (Oracle mode)
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "oceanbase",
        factory: toasty_factory,
        checker: Some(|| Arc::new(OceanBaseValidConnectionChecker::new())),
        sorter: Some(|| Arc::new(OceanBaseOracleExceptionSorter::new())),
    }
}

// ---------------------------------------------------------------------------
// SQLite (default, no special checker/sorter needed)
// ---------------------------------------------------------------------------

inventory::submit! {
    DriverExtensionDescriptor {
        db_type: "sqlite",
        factory: toasty_factory,
        checker: None,
        sorter: None,
    }
}
