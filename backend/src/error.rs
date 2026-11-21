//! Error API sebagai `application/problem+json` (RFC 9457) + extractor yang
//! mengubah penolakan axum menjadi format yang sama.

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use validator::Validate;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub detail: String,
}

impl ApiError {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self { status, detail: detail.into() }
    }
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Token tidak valid atau kedaluwarsa")
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "Data tidak ditemukan")
    }
    /// Catat penyebab di log, balas 500 tanpa detail internal.
    pub fn internal(e: impl std::fmt::Display) -> Self {
        tracing::error!(error = %e, "internal error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Terjadi kesalahan di server")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "type": "about:blank",
            "title": self.status.canonical_reason().unwrap_or("Error"),
            "status": self.status.as_u16(),
            "detail": self.detail,
        });
        (
            self.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            body.to_string(),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        if matches!(e, sqlx::Error::RowNotFound) {
            return Self::not_found();
        }
        if let Some(db) = e.as_database_error() {
            if db.is_unique_violation() {
                return Self::new(StatusCode::CONFLICT, "Data sudah ada");
            }
            if db.is_foreign_key_violation() {
                return Self::bad_request("Referensi ke data yang tidak ada");
            }
        }
        Self::internal(e)
    }
}

/// `Json<T>` + validasi `validator`.
pub struct ValidJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| ApiError::new(e.status(), e.body_text()))?;
        value
            .validate()
            .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
        Ok(Self(value))
    }
}

/// `Query<T>` dengan penolakan sebagai problem JSON.
pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| Self(v))
            .map_err(|e| ApiError::new(e.status(), e.body_text()))
    }
}
