use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use druid_admin::StatViewServlet;
use tower::ServiceExt as _;

#[tokio::test]
async fn stat_view_servlet_preserves_legacy_path_and_json_with_injected_service() {
    let router = StatViewServlet::new("/druid", |url| {
        format!(r#"{{"ResultCode":1,"Content":"{url}"}}"#)
    })
    .router();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/druid/sql.json?dataSourceId=7")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("injected servlet response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/json;charset=utf-8"
    );
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("response bytes");
    assert_eq!(
        body.as_ref(),
        br#"{"ResultCode":1,"Content":"/sql.json?dataSourceId=7"}"#
    );
}
