use axum::extract::State;
use isuride::{AppState, Error};
use moka::future::Cache;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info,tower_http=info,axum::rejection=trace");
    }
    tracing_subscriber::fmt::init();

    let host = std::env::var("ISUCON_DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = std::env::var("ISUCON_DB_PORT")
        .map(|port_str| {
            port_str.parse().expect(
                "failed to convert DB port number from ISUCON_DB_PORT environment variable into u16",
            )
        })
        .unwrap_or(3306);
    let user = std::env::var("ISUCON_DB_USER").unwrap_or_else(|_| "isucon".to_owned());
    let password = std::env::var("ISUCON_DB_PASSWORD").unwrap_or_else(|_| "isucon".to_owned());
    let dbname = std::env::var("ISUCON_DB_NAME").unwrap_or_else(|_| "isuride".to_owned());
    let payment_gateway_url =
        std::env::var("ISUCON_PAYMENT_GATEWAY_URL").unwrap_or("http://localhost:12345".to_string());

    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(50)
        .connect_with(
            sqlx::mysql::MySqlConnectOptions::default()
                .host(&host)
                .port(port)
                .username(&user)
                .password(&password)
                .database(&dbname),
        )
        .await?;
    let chair_cache = Cache::builder()
        // Time to live (TTL): 1 minutes
        .time_to_live(Duration::from_secs(60))
        .build();
    let ride_status_cache = Cache::builder()
        // Time to live (TTL): 1 minutes
        .time_to_live(Duration::from_secs(60))
        .build();

    let app_state = AppState {
        pool,
        chair_cache,
        ride_status_cache,
        payment_gateway_url,
        internal_matching_lock: Arc::new(Semaphore::new(1)),
    };

    let app = axum::Router::new()
        .route("/api/initialize", axum::routing::post(post_initialize))
        .merge(isuride::app_handlers::app_routes(app_state.clone()))
        .merge(isuride::owner_handlers::owner_routes(app_state.clone()))
        .merge(isuride::chair_handlers::chair_routes(app_state.clone()))
        .merge(isuride::internal_handlers::internal_routes())
        .with_state(app_state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let tcp_listener =
        if let Some(std_listener) = listenfd::ListenFd::from_env().take_tcp_listener(0)? {
            TcpListener::from_std(std_listener)?
        } else {
            TcpListener::bind(&SocketAddr::from(([0, 0, 0, 0], 8080))).await?
        };
    axum::serve(tcp_listener, app).await?;

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct PostInitializeRequest {
    payment_server: String,
}

#[derive(Debug, serde::Serialize)]
struct PostInitializeResponse {
    language: &'static str,
}

async fn post_initialize(
    State(AppState {
        pool,
        chair_cache,
        ride_status_cache,
        ..
    }): State<AppState>,
    axum::Json(req): axum::Json<PostInitializeRequest>,
) -> Result<axum::Json<PostInitializeResponse>, Error> {
    let output = tokio::process::Command::new("../sql/init.sh")
        .output()
        .await?;
    if !output.status.success() {
        return Err(Error::Initialize {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    chair_cache.invalidate_all();
    ride_status_cache.invalidate_all();

    sqlx::query("UPDATE settings SET value = ? WHERE name = 'payment_gateway_url'")
        .bind(req.payment_server)
        .execute(&pool)
        .await?;

    Ok(axum::Json(PostInitializeResponse { language: "rust" }))
}
