#!/usr/bin/env python3
"""Compile a repository-external consumer of Task 6 public/config contracts."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


SOURCE = r"""#![allow(deprecated)]

use druid::pool::{DruidPool, PoolInnerConfig};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, ConnectionFactory, Driver, DriverPropertyInfo};
use druid_rdbc::RdbcError;
use druid_wrapper::driver::ResolvedDatabaseDriver;
use std::sync::Arc;
use std::time::Duration;

struct ExternalDriver;

#[async_trait::async_trait]
impl Driver for ExternalDriver {
    fn name(&self) -> &str { "external" }
    fn accepts_url(&self, _url: &str) -> bool { false }
    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Ok(None)
    }
}

fn wrapper_factory(driver: &ResolvedDatabaseDriver) -> Arc<dyn ConnectionFactory> {
    driver.factory()
}

pub fn verify_contract() {
    let config = PoolInnerConfig::default();
    assert_eq!(config.physical_connection_timeout, None);
    let _builder = DruidPool::builder()
        .physical_connection_timeout(Duration::from_secs(30))
        .phy_timeout(Duration::from_secs(60));
    let property = DriverPropertyInfo::new("ssl", None);
    assert_eq!(property.description, None);
    assert!(property.choices.is_empty());
    assert_eq!(ExternalDriver.name(), "external");
    let _signature: fn(&ResolvedDatabaseDriver) -> Arc<dyn ConnectionFactory> = wrapper_factory;
}
"""


def main() -> int:
    """Build the external crate with only path dependencies and no workspace membership."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    with tempfile.TemporaryDirectory(
        prefix="druid-task6-external-public-"
    ) as temporary:
        consumer = Path(temporary)
        manifest = f"""[package]
name = "task6-external-public-contract"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
async-trait = "0.1"
druid = {{ path = "{root / 'crates/druid'}" }}
druid-rdbc = {{ path = "{root / 'crates/druid-rdbc'}" }}
druid-wrapper = {{ path = "{root / 'crates/druid-wrapper'}", default-features = false, features = ["driver-catalog"] }}
sqlx = {{ version = "=0.8.6", default-features = false }}

[patch.crates-io]
toasty-driver-sqlite = {{ path = "{root / 'vendor/toasty-driver-sqlite'}" }}
"""
        (consumer / "src").mkdir()
        (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
        (consumer / "src/lib.rs").write_text(SOURCE, encoding="utf-8", newline="\n")
        result = subprocess.run(
            [
                "cargo",
                "check",
                "--manifest-path",
                str(consumer / "Cargo.toml"),
                "--offline",
            ],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
            timeout=600,
        )
        if result.returncode != 0:
            print(
                "TASK6_EXTERNAL_PUBLIC_CONTRACT_FAIL="
                + (result.stderr or result.stdout).strip()
            )
            return 1
    print("TASK6_EXTERNAL_PUBLIC_CONTRACT=PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
