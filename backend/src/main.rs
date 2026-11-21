use axum::http::{HeaderValue, Method, header};
use expense_backend::{AppState, api, config::Config, parser, sync};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), parser::BoxError> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env()?;
    let pool = sqlx::PgPool::connect(&cfg.database_url).await?;
    sqlx::migrate!().run(&pool).await?;

    let rules = parser::load_rule_configs(include_str!("../config/senders.toml"))?;
    parser::compile_rules(rules.clone())?; // gagal cepat jika ada regex rusak
    sync::seed_global_rules(&pool, &rules).await?;
    // Putaran yang terputus oleh restart tidak boleh mengunci user selamanya.
    sqlx::query("UPDATE sync_state SET status = 'idle' WHERE status = 'syncing'")
        .execute(&pool)
        .await?;

    let cors = CorsLayer::new()
        .allow_origin(cfg.frontend_origin.parse::<HeaderValue>()?)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let addr = cfg.bind_addr.clone();
    if cfg.mock_gmail {
        tracing::warn!("MOCK_GMAIL=true: email dari fixtures/, dev-login aktif. Jangan untuk produksi.");
    }
    let state = AppState { pool, cfg: Arc::new(cfg), http: reqwest::Client::new() };
    tokio::spawn(sync::worker(state.clone()));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(listener, api::router(state).layer(cors)).await?;
    Ok(())
}
