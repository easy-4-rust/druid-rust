//! 把分离前端构建产物内嵌进二进制。
//!
//! 前端是独立的应用：它有自己的构建（`crates/druid-admin/web`，npm + Vite），
//! 与后端只通过版本化 JSON API 通信。构建产物由本脚本读取并生成为静态表，
//! 从而让 Admin 仍然是一个自包含的可执行文件，同时新增前端资源不需要改动
//! 任何 Rust 源码。
//!
//! 前端未构建时生成空表：Admin 仍然可以编译并正常提供 API。

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// 前端构建产物目录。
const DIST: &str = "web/dist";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dist = manifest_dir.join(DIST);
    println!("cargo:rerun-if-changed={}", dist.display());
    println!("cargo:rerun-if-changed=build.rs");

    let mut assets: BTreeMap<String, String> = BTreeMap::new();
    if dist.is_dir() {
        collect(&dist, &dist, &mut assets);
    }

    let mut generated = String::new();
    generated.push_str("// 由 build.rs 生成，请勿手工修改。\n");
    generated.push_str("&[\n");
    for (path, body) in &assets {
        generated.push_str(&format!(
            "    ({:?}, {:?}),\n",
            path,
            body
        ));
    }
    generated.push_str("]\n");

    let out = PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("console_assets.rs");
    fs::write(&out, generated).expect("write console assets");
    println!(
        "cargo:warning=embedded {} console asset(s) from {}",
        assets.len(),
        DIST
    );
}

/// 递归收集目录下的文件。
fn collect(root: &Path, current: &Path, assets: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, assets);
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let key = relative.to_string_lossy().replace('\\', "/");
        // 只嵌入文本资源；前端构建产物不含二进制文件。
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(body) = String::from_utf8(bytes) else {
            continue;
        };
        assets.insert(key, body);
    }
}
