//! 分离前端的分发。
//!
//! 前端是独立构建的应用，资源由构建脚本内嵌进二进制。
//! Admin 只负责把资源发出去，不参与前端的渲染。

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

/// 构建脚本生成的内嵌资源表。
pub static CONSOLE_ASSETS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/console_assets.rs"));

/// 前端入口文件名。
const ENTRY: &str = "index.html";

/// 返回内嵌资源的内容类型。
fn content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    }
}

/// 读取一个内嵌资源。
fn asset(path: &str) -> Option<&'static str> {
    CONSOLE_ASSETS
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, body)| *body)
}

/// 返回前端入口。
#[must_use]
pub fn entry_document() -> Option<&'static str> {
    asset(ENTRY)
}

/// 分发一条前端资源请求。
///
/// 未命中的路径回落到前端入口，由前端路由自行处理，
/// 这正是前后端分离后前端接管路由的方式。
pub(crate) fn serve(path: &str) -> Response {
    let normalized = path.trim_start_matches('/');
    let body = asset(normalized)
        .or_else(|| asset(ENTRY))
        .unwrap_or_default();
    let is_entry = asset(normalized).is_none();
    let status = if body.is_empty() {
        StatusCode::NOT_FOUND
    } else if is_entry && normalized.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::OK
    };
    (
        status,
        [
            (header::CONTENT_TYPE, content_type(normalized)),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// 前端资源是否已经内嵌。
#[must_use]
pub fn is_embedded() -> bool {
    !CONSOLE_ASSETS.is_empty()
}
