//! Sesi backend (access JWT + refresh token opaque), verifikasi ID token Google,
//! dan enkripsi refresh token Gmail. Token tidak pernah di-log.

use crate::{
    AppState,
    error::{ApiError, ValidJson},
    parser::BoxError,
    sync,
};
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, jwk::JwkSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

const ACCESS_TTL_SECS: i64 = 15 * 60;
const REFRESH_TTL_DAYS: i32 = 30;
const GOOGLE_JWKS: &str = "https://www.googleapis.com/oauth2/v3/certs";

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    exp: i64,
}

/// Extractor penjaga endpoint: butuh `Authorization: Bearer <access JWT>`.
pub struct AuthUser(pub Uuid);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(ApiError::unauthorized)?;
        let key = DecodingKey::from_secret(state.cfg.jwt_secret.as_bytes())
            .map_err(ApiError::internal)?;
        jsonwebtoken::decode::<Claims>(token, &key, &Validation::new(Algorithm::HS256))
        .map(|d| Self(d.claims.sub))
        .map_err(|_| ApiError::unauthorized())
    }
}

#[derive(Serialize, ToSchema)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix detik saat access token kedaluwarsa.
    pub expires_at: i64,
}

fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

async fn issue_session(state: &AppState, user_id: Uuid) -> Result<TokenPair, ApiError> {
    let refresh_token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    sqlx::query(
        "INSERT INTO sessions (user_id, refresh_token_hash, expires_at)
         VALUES ($1, $2, now() + make_interval(days => $3))",
    )
    .bind(user_id)
    .bind(hash(&refresh_token))
    .bind(REFRESH_TTL_DAYS)
    .execute(&state.pool)
    .await?;

    let expires_at = chrono::Utc::now().timestamp() + ACCESS_TTL_SECS;
    let access_token = jsonwebtoken::encode(
        &Header::default(),
        &Claims { sub: user_id, exp: expires_at },
        &EncodingKey::from_secret(state.cfg.jwt_secret.as_bytes()).map_err(ApiError::internal)?,
    )
    .map_err(ApiError::internal)?;
    Ok(TokenPair { access_token, refresh_token, expires_at })
}

// ---------- enkripsi refresh token Gmail ----------

pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), BoxError> {
    let nonce = rand::random::<[u8; 12]>();
    let ciphertext = Aes256Gcm::new(key.into())
        .encrypt(&nonce.into(), plaintext.as_bytes())
        .map_err(|_| "enkripsi gagal")?;
    Ok((ciphertext, nonce.to_vec()))
}

pub fn decrypt(key: &[u8; 32], ciphertext: &[u8], nonce: &[u8]) -> Result<String, BoxError> {
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| "nonce harus 12 byte")?;
    let plain = Aes256Gcm::new(key.into())
        .decrypt(&nonce.into(), ciphertext)
        .map_err(|_| "dekripsi gagal")?;
    Ok(String::from_utf8(plain)?)
}

/// Refresh token Gmail milik user, sudah didekripsi.
pub async fn gmail_refresh_token(state: &AppState, user_id: Uuid) -> Result<Option<String>, BoxError> {
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT refresh_token_ciphertext, nonce FROM google_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    row.map(|(c, n)| decrypt(&state.cfg.token_key, &c, &n)).transpose()
}

// ---------- handlers ----------

#[derive(Deserialize, Validate, ToSchema)]
pub struct ExchangeReq {
    #[validate(length(min = 1))]
    id_token: String,
    /// Refresh token Gmail. Boleh kosong jika Google tidak mengirim ulang.
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct GoogleClaims {
    sub: String,
    email: String,
    #[serde(default)]
    email_verified: bool,
    name: Option<String>,
    picture: Option<String>,
}

async fn verify_google_id_token(state: &AppState, id_token: &str) -> Result<GoogleClaims, BoxError> {
    let kid = jsonwebtoken::decode_header(id_token)?.kid.ok_or("ID token tanpa kid")?;
    // ponytail: JWKS diambil tiap login; tambah cache jika login jadi sering.
    let jwks: JwkSet = state.http.get(GOOGLE_JWKS).send().await?.json().await?;
    let jwk = jwks.find(&kid).ok_or("kid tidak dikenal")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&state.cfg.google_client_id]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    let claims =
        jsonwebtoken::decode::<GoogleClaims>(id_token, &DecodingKey::from_jwk(jwk)?, &validation)?
            .claims;
    if !claims.email_verified {
        return Err("email belum terverifikasi".into());
    }
    Ok(claims)
}

async fn upsert_user(
    state: &AppState,
    sub: &str,
    email: &str,
    name: Option<&str>,
    avatar: Option<&str>,
) -> Result<Uuid, ApiError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (google_sub, email, name, avatar_url) VALUES ($1, $2, $3, $4)
         ON CONFLICT (google_sub) DO UPDATE
         SET email = EXCLUDED.email, name = EXCLUDED.name, avatar_url = EXCLUDED.avatar_url
         RETURNING id",
    )
    .bind(sub)
    .bind(email)
    .bind(name)
    .bind(avatar)
    .fetch_one(&state.pool)
    .await?;
    // Login ulang menghidupkan lagi sinkronisasi yang terputus.
    sqlx::query(
        "UPDATE sync_state SET status = 'idle', last_error = NULL
         WHERE user_id = $1 AND status IN ('needs_reauth', 'disconnected')",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(id)
}

fn spawn_sync(state: &AppState, user_id: Uuid) {
    let state = state.clone();
    tokio::spawn(async move { sync::sync_user(&state, user_id).await });
}

#[utoipa::path(post, path = "/api/v1/auth/exchange", tag = "auth", request_body = ExchangeReq,
    responses((status = 200, body = TokenPair), (status = 401, description = "ID token ditolak")))]
pub async fn exchange(
    State(state): State<AppState>,
    ValidJson(req): ValidJson<ExchangeReq>,
) -> Result<Json<TokenPair>, ApiError> {
    let claims = verify_google_id_token(&state, &req.id_token).await.map_err(|e| {
        tracing::warn!(error = %e, "ID token Google ditolak");
        ApiError::unauthorized()
    })?;
    let user_id = upsert_user(
        &state,
        &claims.sub,
        &claims.email,
        claims.name.as_deref(),
        claims.picture.as_deref(),
    )
    .await?;

    if let Some(refresh) = req.refresh_token.filter(|t| !t.is_empty()) {
        let (ciphertext, nonce) =
            encrypt(&state.cfg.token_key, &refresh).map_err(ApiError::internal)?;
        sqlx::query(
            "INSERT INTO google_tokens (user_id, refresh_token_ciphertext, nonce) VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE
             SET refresh_token_ciphertext = EXCLUDED.refresh_token_ciphertext,
                 nonce = EXCLUDED.nonce, updated_at = now()",
        )
        .bind(user_id)
        .bind(ciphertext)
        .bind(nonce)
        .execute(&state.pool)
        .await?;
    }
    spawn_sync(&state, user_id);
    issue_session(&state, user_id).await.map(Json)
}

/// Hanya aktif saat MOCK_GMAIL=true: sesi untuk satu user dev tanpa Google.
#[utoipa::path(post, path = "/api/v1/auth/dev-login", tag = "auth",
    responses((status = 200, body = TokenPair), (status = 404, description = "Bukan mode mock")))]
pub async fn dev_login(State(state): State<AppState>) -> Result<Json<TokenPair>, ApiError> {
    if !state.cfg.mock_gmail {
        return Err(ApiError::not_found());
    }
    let user_id =
        upsert_user(&state, "dev-user", "dev@example.com", Some("User Dev"), None).await?;
    spawn_sync(&state, user_id);
    issue_session(&state, user_id).await.map(Json)
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct RefreshReq {
    #[validate(length(min = 1))]
    refresh_token: String,
}

#[utoipa::path(post, path = "/api/v1/auth/refresh", tag = "auth", request_body = RefreshReq,
    responses((status = 200, body = TokenPair), (status = 401, description = "Refresh token tidak valid")))]
pub async fn refresh(
    State(state): State<AppState>,
    ValidJson(req): ValidJson<RefreshReq>,
) -> Result<Json<TokenPair>, ApiError> {
    // Rotasi: token lama dicabut. Masa tenggang 30 detik agar request paralel
    // dari frontend yang memakai token lama yang sama tidak saling menggagalkan.
    let user_id: Uuid = sqlx::query_scalar(
        "UPDATE sessions SET revoked_at = COALESCE(revoked_at, now())
         WHERE refresh_token_hash = $1 AND expires_at > now()
           AND (revoked_at IS NULL OR revoked_at > now() - interval '30 seconds')
         RETURNING user_id",
    )
    .bind(hash(&req.refresh_token))
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(ApiError::unauthorized)?;
    issue_session(&state, user_id).await.map(Json)
}

#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth", request_body = RefreshReq,
    responses((status = 204)))]
pub async fn logout(
    State(state): State<AppState>,
    ValidJson(req): ValidJson<RefreshReq>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() - interval '1 minute' WHERE refresh_token_hash = $1")
        .bind(hash(&req.refresh_token))
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct Me {
    id: Uuid,
    email: String,
    name: Option<String>,
    avatar_url: Option<String>,
    google_connected: bool,
}

#[utoipa::path(get, path = "/api/v1/auth/me", tag = "auth", security(("bearer" = [])),
    responses((status = 200, body = Me)))]
pub async fn me(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> Result<Json<Me>, ApiError> {
    let me = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.avatar_url,
                (EXISTS (SELECT 1 FROM google_tokens g WHERE g.user_id = u.id)
                 OR ($2 AND COALESCE(s.status, 'idle') <> 'disconnected')) AS google_connected
         FROM users u LEFT JOIN sync_state s ON s.user_id = u.id WHERE u.id = $1",
    )
    .bind(user_id)
    .bind(state.cfg.mock_gmail)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(me))
}

/// Cabut akses Gmail di Google, hapus token, hentikan sinkronisasi. Transaksi tetap ada.
#[utoipa::path(post, path = "/api/v1/auth/disconnect-google", tag = "auth", security(("bearer" = [])),
    responses((status = 204)))]
pub async fn disconnect_google(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<StatusCode, ApiError> {
    if let Ok(Some(token)) = gmail_refresh_token(&state, user_id).await {
        // Best effort: token tetap dihapus lokal walau revoke gagal.
        let revoke = state
            .http
            .post("https://oauth2.googleapis.com/revoke")
            .form(&[("token", token.as_str())])
            .send()
            .await;
        if let Err(e) = revoke {
            tracing::warn!(error = %e.without_url(), "revoke token Google gagal");
        }
    }
    sqlx::query("DELETE FROM google_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        "INSERT INTO sync_state (user_id, status) VALUES ($1, 'disconnected')
         ON CONFLICT (user_id) DO UPDATE SET status = 'disconnected', history_id = NULL",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_roundtrip_and_tamper() {
        let key = [7u8; 32];
        let (ciphertext, nonce) = encrypt(&key, "1//refresh-token").unwrap();
        assert_ne!(ciphertext, b"1//refresh-token");
        assert_eq!(decrypt(&key, &ciphertext, &nonce).unwrap(), "1//refresh-token");
        assert!(decrypt(&[8u8; 32], &ciphertext, &nonce).is_err());
        let mut bad = ciphertext.clone();
        bad[0] ^= 1;
        assert!(decrypt(&key, &bad, &nonce).is_err());
    }
}
