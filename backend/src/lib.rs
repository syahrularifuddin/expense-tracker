pub mod api;
pub mod auth;
pub mod config;
pub mod error;
pub mod gmail;
pub mod parser;
pub mod sync;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub cfg: Arc<config::Config>,
    pub http: reqwest::Client,
}
