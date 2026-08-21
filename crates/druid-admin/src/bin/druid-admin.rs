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
//! - `DRUID_ADMIN_HTTP_ADDR` - HTTP bind address (default: `127.0.0.1:8080`)
//! - `DRUID_ADMIN_GRPC_ADDR` - gRPC bind address (default: `127.0.0.1:9090`)
//! - `DRUID_ADMIN_TLS_CERT_PATH` / `DRUID_ADMIN_TLS_KEY_PATH` - shared PEM material
//! - `DRUID_ADMIN_AUTH_MODE` - `disabled` (loopback only) or `basic`
//! - `DRUID_ADMIN_LOGIN_USERNAME` / `DRUID_ADMIN_LOGIN_PASSWORD` - Basic credentials

use std::sync::Arc;
use std::time::Duration;

use axum::serve::Listener;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Server, ServerTlsConfig};
use tracing_subscriber::EnvFilter;

use druid_admin::config::AdminServerConfig;
use druid_admin::repository::MetricsRepository;
use druid_metrics::protocol::metrics_ingest_server::MetricsIngestServer;

const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Configuration from environment
    let config = AdminServerConfig::from_env()?;
    let tls_material = config.load_tls_material()?;
    let http_addr = config.http_addr();
    let grpc_addr = config.grpc_addr();

    tracing::info!(%http_addr, %grpc_addr, "starting druid-admin");

    // Shared repository
    let repo = MetricsRepository::new();
    let shutdown_token = CancellationToken::new();

    // Build HTTP router
    let http_app =
        druid_admin::production_router(repo.clone(), config.auth().clone(), tls_material.is_some());
    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    let http_shutdown = shutdown_token.clone();
    let http_tls = tls_material.clone();

    let mut http_handle = tokio::spawn(async move {
        if let Some(tls) = http_tls {
            let listener = TlsListener::new(http_listener, tls.http_server_config());
            axum::serve(listener, http_app)
                .with_graceful_shutdown(http_shutdown.cancelled_owned())
                .await
        } else {
            axum::serve(http_listener, http_app)
                .with_graceful_shutdown(http_shutdown.cancelled_owned())
                .await
        }
    });

    // Build gRPC server
    let grpc_shutdown = shutdown_token.clone();
    let ingest_service = druid_admin::ingest::IngestService::new(repo.clone())
        .with_shutdown_token(grpc_shutdown.clone());
    let grpc_auth = config.auth().clone();
    let grpc_identity = tls_material
        .as_ref()
        .map(druid_admin::config::AdminTlsMaterial::grpc_identity);

    let mut grpc_handle = tokio::spawn(async move {
        let service = MetricsIngestServer::with_interceptor(ingest_service, move |request| {
            grpc_auth.authorize_grpc_request(request)
        });
        let mut server = Server::builder();
        if let Some(identity) = grpc_identity {
            server = server.tls_config(
                ServerTlsConfig::new()
                    .identity(identity)
                    .timeout(TLS_HANDSHAKE_TIMEOUT),
            )?;
        }
        server
            .add_service(service)
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

struct TlsListener {
    listener: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}

impl TlsListener {
    fn new(listener: tokio::net::TcpListener, config: Arc<rustls::ServerConfig>) -> Self {
        Self {
            listener,
            acceptor: tokio_rustls::TlsAcceptor::from(config),
        }
    }
}

impl Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, address) = match self.listener.accept().await {
                Ok(connection) => connection,
                Err(error) => {
                    tracing::error!(%error, "HTTP TLS listener accept failed");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, self.acceptor.accept(stream)).await {
                Ok(Ok(stream)) => return (stream, address),
                Ok(Err(error)) => {
                    tracing::warn!(%error, "HTTP TLS handshake rejected");
                }
                Err(_) => tracing::warn!("HTTP TLS handshake timed out"),
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.listener.local_addr()
    }
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
