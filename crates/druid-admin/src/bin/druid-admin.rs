//! Standalone Druid Admin binary.
//!
//! Runs two servers on separate ports:
//! - HTTP (Axum) on port 8080: serves REST API, /health, /metrics, static UI
//! - gRPC (Tonic) on port 9090: receives metrics pushes from druid-metrics runtime
//!
//! Both servers share an in-memory [`MetricsRepository`].
//!
//! # Environment Variables
//!
//! - `DRUID_ADMIN_HTTP_ADDR` - HTTP bind address (default: `0.0.0.0:8080`)
//! - `DRUID_ADMIN_GRPC_ADDR` - gRPC bind address (default: `0.0.0.0:9090`)

use std::net::SocketAddr;

use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use druid_admin::repository::MetricsRepository;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Configuration from environment
    let http_addr: SocketAddr = std::env::var("DRUID_ADMIN_HTTP_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_owned())
        .parse()?;
    let grpc_addr: SocketAddr = std::env::var("DRUID_ADMIN_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_owned())
        .parse()?;

    tracing::info!(%http_addr, %grpc_addr, "starting druid-admin");

    // Shared repository
    let repo = MetricsRepository::new();
    let shutdown_token = CancellationToken::new();

    // Build HTTP router
    let http_app = druid_admin::standalone_router(repo.clone());
    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    let http_shutdown = shutdown_token.clone();

    let mut http_handle = tokio::spawn(async move {
        axum::serve(http_listener, http_app)
            .with_graceful_shutdown(async move {
                http_shutdown.cancelled().await;
            })
            .await
    });

    // Build gRPC server
    let grpc_shutdown = shutdown_token.clone();
    let ingest_service = druid_admin::ingest::IngestService::new(repo.clone())
        .with_shutdown_token(grpc_shutdown.clone());

    let mut grpc_handle = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(ingest_service.into_server())
            .serve_with_shutdown(grpc_addr, grpc_shutdown.cancelled_owned())
            .await
    });

    enum FirstExit {
        Signal,
        Http(Result<Result<(), std::io::Error>, tokio::task::JoinError>),
        Grpc(Result<Result<(), tonic::transport::Error>, tokio::task::JoinError>),
    }

    let first_exit = tokio::select! {
        biased;
        result = &mut http_handle => FirstExit::Http(result),
        result = &mut grpc_handle => FirstExit::Grpc(result),
        () = shutdown_signal() => FirstExit::Signal,
    };
    shutdown_token.cancel();
    match first_exit {
        FirstExit::Signal => {
            tracing::info!("shutdown signal received, stopping servers");
            let (http_result, grpc_result) = tokio::join!(http_handle, grpc_handle);
            http_result??;
            grpc_result??;
        }
        FirstExit::Http(http_result) => {
            let grpc_result = grpc_handle.await;
            http_result??;
            grpc_result??;
            return Err("HTTP server exited before a shutdown signal".into());
        }
        FirstExit::Grpc(grpc_result) => {
            let http_result = http_handle.await;
            http_result??;
            grpc_result??;
            return Err("gRPC server exited before a shutdown signal".into());
        }
    }

    tracing::info!("druid-admin stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
