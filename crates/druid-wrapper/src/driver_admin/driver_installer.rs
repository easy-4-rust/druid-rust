use super::{
    DriverBundleInstallRequest, DriverInstallRequest, DriverInstallerError, InstalledDriver,
    JavaRuntimeInstallation,
};
use crate::driver::{DatabaseProfileId, DriverRuntimeMode, DruidDriverRegistry};
use crate::jdbc_agent::JdbcAgentOptions;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Write;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

mod install_and_resolve;
mod verify_and_manifest;

/// 显式下载或导入 JDBC JAR 的内容寻址安装器。
#[derive(Debug, Clone)]
pub struct DriverInstaller {
    root: PathBuf,
    http_client: reqwest::Client,
    offline: bool,
}

/// 持有跨进程文件锁直到当前安装或校验操作结束。
pub(crate) struct ArtifactLock(Arc<File>);

impl Drop for ArtifactLock {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 1 {
            let _ = FileExt::unlock(self.0.as_ref());
        }
    }
}
