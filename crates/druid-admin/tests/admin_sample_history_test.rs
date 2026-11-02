mod common;

use druid_admin::repository::MetricsRepository;
use serde_json::Value;

#[test]
fn accepted_samples_keep_identity_and_deltas_without_recording_rejected_batches() {
    let repo = MetricsRepository::new();
    let (id, mut first) =
        common::make_datasource_entry_with_sql(7, "sqlite", 1, 11, "select secret", 2);
    first.datasource.active_count = 3;
    first.datasource.wait_thread_count = 1;
    assert!(repo.upsert_datasource(id.clone(), first.clone()));
    assert!(!repo.upsert_datasource(id.clone(), first.clone()));
    first.sequence += 1;
    first.datasource.active_count = 4;
    first.datasource.wait_thread_count = 0;
    first.sql_stats.get_mut(&11).unwrap().execute_count = 5;
    assert!(repo.upsert_datasource(id.clone(), first));
    let samples: Vec<Value> = repo.sample_history(None, None, 50);
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0]["activeCount"], 3);
    assert_eq!(samples[1]["activeCount"], 4);
    assert_eq!(samples[1]["executeCountDelta"], 3);
    assert!(samples[0]["executeCountDelta"].is_null());
    assert!(!samples[1].to_string().contains("select secret"));
    let scoped = repo.sample_history(
        Some(id.service_identity.shared_service_id().as_ref()),
        Some(7),
        1,
    );
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0]["sequence"], 2);
}

#[test]
fn history_is_bounded_and_counter_reset_is_not_a_negative_rate() {
    let repo = MetricsRepository::new();
    let (id, mut entry) = common::make_datasource_entry_with_sql(7, "sqlite", 1, 11, "fp", 8);
    for sequence in 1..=4_100 {
        entry.sequence = sequence;
        entry.sql_stats.get_mut(&11).unwrap().execute_count =
            if sequence == 2 { 0 } else { sequence as i64 };
        assert!(repo.upsert_datasource(id.clone(), entry.clone()));
        if sequence == 2 {
            assert!(repo.sample_history(None, None, 1)[0]["executeCountDelta"].is_null());
        }
    }
    let samples = repo.sample_history(None, None, 5_000);
    assert!(samples.len() <= 4_096);
    assert_eq!(samples.last().unwrap()["sequence"], 4_100);
    let first_page = repo.sample_history(None, None, 1);
    assert_eq!(first_page.len(), 1);
}

#[test]
fn history_expiry_and_clear_reclaim_samples() {
    use druid_admin::repository::MetricsRepositoryConfig;
    let config =
        MetricsRepositoryConfig::try_new(2, 2, 2, std::time::Duration::from_millis(5), 1024)
            .unwrap();
    let repo = MetricsRepository::with_config(config);
    let (id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    assert!(repo.upsert_datasource(id, entry));
    assert_eq!(repo.sample_history_status()["storedSamples"], 1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
    while repo.sample_history_status()["storedSamples"] != 0 && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(repo.sample_history_status()["storedSamples"], 0);
    repo.clear();
    assert_eq!(repo.sample_history_status()["storedSamples"], 0);
}

#[tokio::test]
async fn history_http_filters_identity_and_reports_process_local_storage() {
    let repo = MetricsRepository::new();
    let (id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    assert!(repo.upsert_datasource(id.clone(), entry));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = druid_admin::MonitorViewServlet::from_repository(repo).router();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let base = format!("http://{address}/druid/api/v1/history");
    let response = reqwest::get(format!("{base}?datasourceId=7&limit=50000"))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["samples"].as_array().unwrap().len(), 1);
    assert_eq!(body["storage"]["durable"], false);
    assert_eq!(body["storage"]["maxSamples"], 4096);
    assert_eq!(
        reqwest::get(format!("{base}?datasourceId=invalid"))
            .await
            .unwrap()
            .status(),
        422
    );
    task.abort();
}

#[tokio::test]
async fn history_route_uses_the_existing_management_authentication_boundary() {
    let repo = MetricsRepository::new();
    let (id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    assert!(repo.upsert_datasource(id, entry));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = druid_admin::MonitorViewServlet::from_repository(repo)
        .with_auth_mode(druid_admin::config::AdminAuthMode::basic("admin", "secret"))
        .router();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let url = format!("http://{address}/druid/api/v1/history");
    assert_eq!(client.get(&url).send().await.unwrap().status(), 302);
    let response = client
        .get(&url)
        .basic_auth("admin", Some("secret"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["samples"].as_array().unwrap().len(), 1);
    task.abort();
}

#[tokio::test]
async fn collection_health_reports_actual_process_memory_when_available() {
    let sampler_available = memory_stats::memory_stats().is_some();
    let repo = MetricsRepository::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = druid_admin::MonitorViewServlet::from_repository(repo).router();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let health: Value = reqwest::get(format!("http://{address}/druid/api/v1/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    if sampler_available {
        assert!(health["processRssBytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0));
        assert!(!health["unavailableFields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "processRssBytes"));
    } else {
        assert!(health["processRssBytes"].is_null());
        assert!(health["unavailableFields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "processRssBytes"));
    }
    task.abort();
}
