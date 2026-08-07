use druid_admin::driver::{DriverInstallRequest, DriverInstaller, DriverRuntimeDiagnostics};
use druid_wrapper::driver::DruidDriverRegistry;
use serde_json::json;
use std::error::Error;
use std::path::PathBuf;

/// 显式管理 Druid JDBC Agent 与驱动 JAR，不在核心建池路径访问网络。
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "catalog" => catalog()?,
        "list" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let installer = DriverInstaller::new(root);
            let active = installer.active_installation(&profile_id).await.ok();
            let versions = installer.installations(&profile_id).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "profileId": profile_id,
                    "activeSha256": active.as_ref().map(|value| value.sha256()),
                    "versions": versions,
                }))?
            );
        }
        "install-agent" => {
            let root = required(&mut arguments, "root")?;
            let source = required(&mut arguments, "agent-jar")?;
            let checksum = arguments.next();
            let installed = DriverInstaller::new(root)
                .install_agent_file(source, checksum.as_deref())
                .await?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        "install-jre" => {
            let root = required(&mut arguments, "root")?;
            let java_home = required(&mut arguments, "java-home")?;
            let checksum = required(&mut arguments, "java-executable-sha256")?;
            let installed = DriverInstaller::new(root)
                .register_java_runtime(java_home, &checksum)
                .await?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        "verify-jre" => {
            let root = required(&mut arguments, "root")?;
            let installed = DriverInstaller::new(root).active_java_runtime().await?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        "install" | "install-file" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let source = required(&mut arguments, "driver-jar")?;
            let mut request = DriverInstallRequest::new(profile_id, source);
            if let Some(checksum) = arguments.next() {
                request = request.expected_sha256(checksum);
            }
            let installed = DriverInstaller::new(root).install_file(&request).await?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        "install-url" | "update" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let url = required(&mut arguments, "url")?;
            let file_name = required(&mut arguments, "file-name")?;
            let checksum = required(&mut arguments, "sha256")?;
            let installed = DriverInstaller::new(root)
                .install_url(&profile_id, &url, &file_name, &checksum)
                .await?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        "verify" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let installer = DriverInstaller::new(root);
            let sha256 = match arguments.next() {
                Some(sha256) => sha256,
                None => installer
                    .active_installation(&profile_id)
                    .await?
                    .sha256()
                    .to_owned(),
            };
            let verified = installer.verify_installation(&profile_id, &sha256).await?;
            println!("{}", serde_json::to_string_pretty(&verified)?);
        }
        "activate" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let sha256 = required(&mut arguments, "sha256")?;
            let activated = DriverInstaller::new(root)
                .activate_version(&profile_id, &sha256)
                .await?;
            println!("{}", serde_json::to_string_pretty(&activated)?);
        }
        "remove" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let sha256 = required(&mut arguments, "sha256")?;
            DriverInstaller::new(root)
                .remove_version(&profile_id, &sha256)
                .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "profileId": profile_id,
                    "sha256": sha256,
                    "removed": true,
                }))?
            );
        }
        "doctor" => {
            let root = required(&mut arguments, "root")?;
            let profile_id = required(&mut arguments, "profile-id")?;
            let mut diagnostics = DriverRuntimeDiagnostics::new(DriverInstaller::new(root));
            if let Some(java_program) = arguments.next() {
                diagnostics = diagnostics.java_program(java_program);
            }
            let report = diagnostics.check(&profile_id).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "help" | "--help" | "-h" => print_help(),
        other => return Err(format!("unknown command '{other}'").into()),
    }
    Ok(())
}

fn catalog() -> Result<(), Box<dyn Error>> {
    let registry = DruidDriverRegistry::builtin()?;
    let profiles = registry
        .profiles()
        .map(|profile| {
            json!({
                "id": profile.id().as_str(),
                "displayName": profile.display_name(),
                "dbType": profile.db_type().as_str(),
                "protocolFamily": format!("{:?}", profile.protocol_family()),
                "runtimeMode": format!("{:?}", profile.runtime_mode()),
                "supportStatus": format!("{:?}", profile.support_status()),
                "countsAsSupported": profile.support_status().counts_as_supported(),
                "deliveryPhase": profile.delivery_phase(),
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "catalogVersion": registry.catalog_version(),
            "catalogSize": registry.catalog_size(),
            "supportedCount": registry.supported_count(),
            "profiles": profiles
        }))?
    );
    Ok(())
}

fn required(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    arguments
        .next()
        .ok_or_else(|| format!("missing required argument <{name}>").into())
}

fn print_help() {
    let default_root =
        DriverInstaller::default_root().unwrap_or_else(|_| PathBuf::from(".druid-rust/drivers"));
    println!(
        "druid-driver commands:\n\
         catalog\n\
         list <root> <profile-id>\n\
         install-agent <root> <agent-jar> [sha256]\n\
         install-jre <root> <java-home> <java-executable-sha256>\n\
         verify-jre <root>\n\
         install <root> <profile-id> <driver-jar> [sha256]\n\
         update <root> <profile-id> <url> <file-name> <sha256>\n\
         verify <root> <profile-id> [sha256]\n\
         activate <root> <profile-id> <sha256>\n\
         remove <root> <profile-id> <sha256>\n\
         doctor <root> <profile-id> [java-program]\n\
         default root: {}",
        default_root.display()
    );
}
