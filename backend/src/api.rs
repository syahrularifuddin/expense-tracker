//! Router + handler REST `/api/v1` (selain auth) dan dokumen OpenAPI.

use crate::{
    AppState,
    auth::{self, AuthUser},
    error::{ApiError, ApiQuery, ValidJson},
    sync,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use chrono::{DateTime, Datelike, FixedOffset, Months, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, Postgres, QueryBuilder};
use utoipa::{
    IntoParams, Modify, OpenApi, ToSchema,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;
use validator::Validate;

type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn router(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/healthz", get(healthz))
        .route("/auth/exchange", post(auth::exchange))
        .route("/auth/dev-login", post(auth::dev_login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/disconnect-google", post(auth::disconnect_google))
        .route("/transactions", get(list_transactions).post(create_transaction))
        .route("/transactions/{id}", patch(update_transaction).delete(delete_transaction))
        .route("/categories", get(list_categories).post(create_category))
        .route("/categories/{id}", patch(update_category).delete(delete_category))
        .route("/senders", get(list_senders).post(add_sender).patch(toggle_sender))
        .route("/senders/{sender}", axum::routing::delete(delete_sender))
        .route("/unparsed-emails", get(list_unparsed))
        .route("/unparsed-emails/{id}", patch(update_unparsed))
        .route("/reports/summary", get(summary))
        .route("/reports/trends", get(trends))
        .route("/reports/card-bills", get(card_bills))
        .route("/sync", post(trigger_sync))
        .route("/sync/status", get(sync_status));

    Router::new()
        .nest("/api/v1", v1)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback(|| async { ApiError::not_found() })
        .with_state(state)
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Expense Tracker API", version = "0.1.0"),
    paths(
        healthz, auth::exchange, auth::dev_login, auth::refresh, auth::logout, auth::me,
        auth::disconnect_google, list_transactions, create_transaction, update_transaction,
        delete_transaction, list_categories, create_category, update_category, delete_category,
        list_senders, add_sender, toggle_sender, delete_sender, list_unparsed, update_unparsed,
        summary, trends, card_bills, trigger_sync, sync_status
    ),
    modifiers(&BearerAuth)
)]
struct ApiDoc;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi.components.get_or_insert_default().add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new().scheme(HttpAuthScheme::Bearer).bearer_format("JWT").build(),
            ),
        );
    }
}

#[utoipa::path(get, path = "/api/v1/healthz", tag = "health",
    responses((status = 200), (status = 503, description = "DB tidak terjangkau")))]
async fn healthz(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[derive(Serialize, ToSchema)]
pub struct Page<T> {
    items: Vec<T>,
    total: i64,
    page: i64,
    per_page: i64,
}

fn paging(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (page.unwrap_or(1).max(1), per_page.unwrap_or(20).clamp(1, 100))
}

/// Bedakan field tidak dikirim (`None`) dari `null` (`Some(None)`).
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

fn validate_type(t: &str) -> Result<(), validator::ValidationError> {
    if t == "debit" || t == "credit" {
        Ok(())
    } else {
        Err(validator::ValidationError::new("type harus debit atau credit"))
    }
}

// ============================ transactions ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct Transaction {
    id: Uuid,
    occurred_at: DateTime<Utc>,
    /// Dalam sen.
    amount_minor: i64,
    currency: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    tx_type: String,
    merchant: Option<String>,
    description: Option<String>,
    category_id: Option<Uuid>,
    category_name: Option<String>,
    category_color: Option<String>,
    category_locked: bool,
    source: String,
    /// Metode pembayaran: `bca`, `jenius`, `krom`, `custom`, atau `manual`.
    provider: String,
    raw_subject: Option<String>,
}

const TX_SELECT: &str = "SELECT t.id, t.occurred_at, t.amount_minor, t.currency, t.type, t.merchant,
        t.description, t.category_id, c.name AS category_name, c.color AS category_color,
        t.category_locked, t.source, t.provider, t.raw_subject
    FROM transactions t LEFT JOIN categories c ON c.id = t.category_id";

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TxQuery {
    /// Tanggal awal (inklusif, Asia/Jakarta), `YYYY-MM-DD`.
    from: Option<NaiveDate>,
    /// Tanggal akhir (inklusif, Asia/Jakarta), `YYYY-MM-DD`.
    to: Option<NaiveDate>,
    category_id: Option<Uuid>,
    /// `gmail` atau `manual`.
    source: Option<String>,
    /// Metode pembayaran: `bca`, `jenius`, `krom`, `custom`, atau `manual`.
    provider: Option<String>,
    /// `debit` atau `credit`.
    r#type: Option<String>,
    /// Cari di merchant / deskripsi.
    q: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

fn push_tx_filters(qb: &mut QueryBuilder<Postgres>, user_id: Uuid, f: &TxQuery) {
    qb.push(" WHERE t.user_id = ").push_bind(user_id);
    if let Some(from) = f.from {
        qb.push(" AND t.occurred_at >= (").push_bind(from).push("::date)::timestamp AT TIME ZONE 'Asia/Jakarta'");
    }
    if let Some(to) = f.to {
        qb.push(" AND t.occurred_at < (").push_bind(to).push("::date + 1)::timestamp AT TIME ZONE 'Asia/Jakarta'");
    }
    if let Some(c) = f.category_id {
        qb.push(" AND t.category_id = ").push_bind(c);
    }
    if let Some(s) = f.source.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND t.source = ").push_bind(s.to_string());
    }
    if let Some(p) = f.provider.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND t.provider = ").push_bind(p.to_string());
    }
    if let Some(t) = f.r#type.as_deref().filter(|s| !s.is_empty()) {
        qb.push(" AND t.type = ").push_bind(t.to_string());
    }
    if let Some(q) = f.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        qb.push(" AND (t.merchant ILIKE ").push_bind(like.clone());
        qb.push(" OR t.description ILIKE ").push_bind(like).push(")");
    }
}

#[utoipa::path(get, path = "/api/v1/transactions", tag = "transactions", params(TxQuery),
    security(("bearer" = [])), responses((status = 200, body = Page<Transaction>)))]
async fn list_transactions(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ApiQuery(f): ApiQuery<TxQuery>,
) -> ApiResult<Page<Transaction>> {
    let (page, per_page) = paging(f.page, f.per_page);

    let mut count = QueryBuilder::new("SELECT count(*) FROM transactions t");
    push_tx_filters(&mut count, user_id, &f);
    let total: i64 = count.build_query_scalar().fetch_one(&state.pool).await?;

    let mut list = QueryBuilder::new(TX_SELECT);
    push_tx_filters(&mut list, user_id, &f);
    list.push(" ORDER BY t.occurred_at DESC, t.id LIMIT ").push_bind(per_page);
    list.push(" OFFSET ").push_bind((page - 1) * per_page);
    let items = list.build_query_as().fetch_all(&state.pool).await?;

    Ok(Json(Page { items, total, page, per_page }))
}

async fn fetch_transaction(state: &AppState, user_id: Uuid, id: Uuid) -> ApiResult<Transaction> {
    let mut qb = QueryBuilder::new(TX_SELECT);
    qb.push(" WHERE t.user_id = ").push_bind(user_id).push(" AND t.id = ").push_bind(id);
    Ok(Json(qb.build_query_as().fetch_one(&state.pool).await?))
}

/// Kategori harus default atau milik user.
async fn check_category(state: &AppState, user_id: Uuid, category_id: Option<Uuid>) -> Result<(), ApiError> {
    let Some(id) = category_id else { return Ok(()) };
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM categories WHERE id = $1 AND (user_id IS NULL OR user_id = $2))",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    if ok { Ok(()) } else { Err(ApiError::bad_request("Kategori tidak ditemukan")) }
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateTransaction {
    occurred_at: DateTime<Utc>,
    #[validate(range(min = 1))]
    amount_minor: i64,
    #[serde(rename = "type")]
    #[validate(custom(function = "validate_type"))]
    tx_type: String,
    #[validate(length(max = 200))]
    merchant: Option<String>,
    #[validate(length(max = 1000))]
    description: Option<String>,
    category_id: Option<Uuid>,
}

#[utoipa::path(post, path = "/api/v1/transactions", tag = "transactions", request_body = CreateTransaction,
    security(("bearer" = [])), responses((status = 201, body = Transaction)))]
async fn create_transaction(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidJson(req): ValidJson<CreateTransaction>,
) -> Result<(StatusCode, Json<Transaction>), ApiError> {
    check_category(&state, user_id, req.category_id).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO transactions (user_id, occurred_at, amount_minor, type, merchant, description,
                                   category_id, category_locked, source, provider)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7 IS NOT NULL, 'manual', 'manual') RETURNING id",
    )
    .bind(user_id)
    .bind(req.occurred_at)
    .bind(req.amount_minor)
    .bind(&req.tx_type)
    .bind(&req.merchant)
    .bind(&req.description)
    .bind(req.category_id)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, fetch_transaction(&state, user_id, id).await?))
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateTransaction {
    occurred_at: Option<DateTime<Utc>>,
    #[validate(range(min = 1))]
    amount_minor: Option<i64>,
    #[serde(rename = "type")]
    #[validate(custom(function = "validate_type"))]
    tx_type: Option<String>,
    #[validate(length(max = 200))]
    merchant: Option<String>,
    #[validate(length(max = 1000))]
    description: Option<String>,
    /// Dikirim (termasuk `null`) = override manual; kategori otomatis tidak lagi menyentuh baris ini.
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<Uuid>)]
    category_id: Option<Option<Uuid>>,
}

#[utoipa::path(patch, path = "/api/v1/transactions/{id}", tag = "transactions",
    params(("id" = Uuid, Path,)), request_body = UpdateTransaction,
    security(("bearer" = [])), responses((status = 200, body = Transaction)))]
async fn update_transaction(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(req): ValidJson<UpdateTransaction>,
) -> ApiResult<Transaction> {
    check_category(&state, user_id, req.category_id.flatten()).await?;
    let done = sqlx::query(
        "UPDATE transactions SET
            occurred_at  = COALESCE($3, occurred_at),
            amount_minor = COALESCE($4, amount_minor),
            type         = COALESCE($5, type),
            merchant     = COALESCE($6, merchant),
            description  = COALESCE($7, description),
            category_id  = CASE WHEN $8 THEN $9 ELSE category_id END,
            category_locked = category_locked OR $8
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .bind(req.occurred_at)
    .bind(req.amount_minor)
    .bind(&req.tx_type)
    .bind(&req.merchant)
    .bind(&req.description)
    .bind(req.category_id.is_some())
    .bind(req.category_id.flatten())
    .execute(&state.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::not_found());
    }
    fetch_transaction(&state, user_id, id).await
}

#[utoipa::path(delete, path = "/api/v1/transactions/{id}", tag = "transactions",
    params(("id" = Uuid, Path,)), security(("bearer" = [])), responses((status = 204)))]
async fn delete_transaction(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("DELETE FROM transactions WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    if done.rows_affected() == 0 { Err(ApiError::not_found()) } else { Ok(StatusCode::NO_CONTENT) }
}

// ============================ categories ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct Category {
    id: Uuid,
    name: String,
    color: Option<String>,
    /// Kategori default tidak bisa diubah atau dihapus.
    is_default: bool,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct CategoryReq {
    #[validate(length(min = 1, max = 50))]
    name: String,
    #[validate(length(min = 4, max = 9))]
    color: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/categories", tag = "categories", security(("bearer" = [])),
    responses((status = 200, body = Vec<Category>)))]
async fn list_categories(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<Vec<Category>> {
    let rows = sqlx::query_as(
        "SELECT id, name, color, user_id IS NULL AS is_default FROM categories
         WHERE user_id IS NULL OR user_id = $1 ORDER BY user_id IS NULL DESC, name",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/v1/categories", tag = "categories", request_body = CategoryReq,
    security(("bearer" = [])), responses((status = 201, body = Category)))]
async fn create_category(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidJson(req): ValidJson<CategoryReq>,
) -> Result<(StatusCode, Json<Category>), ApiError> {
    let row = sqlx::query_as(
        "INSERT INTO categories (user_id, name, color) VALUES ($1, $2, $3)
         RETURNING id, name, color, false AS is_default",
    )
    .bind(user_id)
    .bind(req.name.trim())
    .bind(&req.color)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[utoipa::path(patch, path = "/api/v1/categories/{id}", tag = "categories",
    params(("id" = Uuid, Path,)), request_body = CategoryReq,
    security(("bearer" = [])), responses((status = 200, body = Category)))]
async fn update_category(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(req): ValidJson<CategoryReq>,
) -> ApiResult<Category> {
    // Kategori default (user_id NULL) tidak cocok dengan filter -> 404.
    let row = sqlx::query_as(
        "UPDATE categories SET name = $3, color = COALESCE($4, color) WHERE id = $1 AND user_id = $2
         RETURNING id, name, color, false AS is_default",
    )
    .bind(id)
    .bind(user_id)
    .bind(req.name.trim())
    .bind(&req.color)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(delete, path = "/api/v1/categories/{id}", tag = "categories",
    params(("id" = Uuid, Path,)), security(("bearer" = [])), responses((status = 204)))]
async fn delete_category(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("DELETE FROM categories WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&state.pool)
        .await?;
    if done.rows_affected() == 0 { Err(ApiError::not_found()) } else { Ok(StatusCode::NO_CONTENT) }
}

// ============================ senders ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct Sender {
    sender: String,
    provider: String,
    /// Sender bawaan (dari senders.toml); hanya bisa dimatikan, tidak dihapus.
    is_global: bool,
    enabled: bool,
}

#[utoipa::path(get, path = "/api/v1/senders", tag = "senders", security(("bearer" = [])),
    responses((status = 200, body = Vec<Sender>)))]
async fn list_senders(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<Vec<Sender>> {
    let rows = sqlx::query_as(
        "SELECT DISTINCT r.sender, r.provider, true AS is_global,
                NOT EXISTS (SELECT 1 FROM disabled_senders d
                            WHERE d.user_id = $1 AND d.sender = r.sender) AS enabled
         FROM parser_rules r WHERE r.user_id IS NULL AND r.sender <> ''
         UNION ALL
         SELECT r.sender, r.provider, false, r.enabled FROM parser_rules r WHERE r.user_id = $1
         ORDER BY 3 DESC, 1",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct SenderReq {
    /// Alamat atau domain pengirim, mis. `bni.co.id`.
    #[validate(length(min = 3, max = 100))]
    sender: String,
}

/// Nilai ini masuk ke query pencarian Gmail, jadi hanya karakter alamat email.
fn clean_sender(raw: &str) -> Result<String, ApiError> {
    let s = raw.trim().to_lowercase();
    if s.contains('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || "@._-".contains(c)) {
        Ok(s)
    } else {
        Err(ApiError::bad_request("Sender harus berupa alamat email atau domain"))
    }
}

#[utoipa::path(post, path = "/api/v1/senders", tag = "senders", request_body = SenderReq,
    security(("bearer" = [])), responses((status = 201), (status = 409, description = "Sudah ada")))]
async fn add_sender(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidJson(req): ValidJson<SenderReq>,
) -> Result<StatusCode, ApiError> {
    let sender = clean_sender(&req.sender)?;
    // Sender milik user memakai regex aturan generik.
    let done = sqlx::query(
        "INSERT INTO parser_rules (user_id, provider, sender, subject_pattern, body_regex, priority)
         SELECT $1, 'custom', $2, g.subject_pattern, g.body_regex, -50
         FROM parser_rules g
         WHERE g.user_id IS NULL AND g.provider = 'generic'
           AND NOT EXISTS (SELECT 1 FROM parser_rules x
                           WHERE x.sender = $2 AND (x.user_id IS NULL OR x.user_id = $1))
         LIMIT 1",
    )
    .bind(user_id)
    .bind(&sender)
    .execute(&state.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, "Sender sudah ada"));
    }
    Ok(StatusCode::CREATED)
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct ToggleSender {
    #[validate(length(min = 1, max = 100))]
    sender: String,
    enabled: bool,
}

#[utoipa::path(patch, path = "/api/v1/senders", tag = "senders", request_body = ToggleSender,
    security(("bearer" = [])), responses((status = 204)))]
async fn toggle_sender(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ValidJson(req): ValidJson<ToggleSender>,
) -> Result<StatusCode, ApiError> {
    let own = sqlx::query("UPDATE parser_rules SET enabled = $3 WHERE user_id = $1 AND sender = $2")
        .bind(user_id)
        .bind(&req.sender)
        .bind(req.enabled)
        .execute(&state.pool)
        .await?;
    if own.rows_affected() == 0 {
        let sql = if req.enabled {
            "DELETE FROM disabled_senders WHERE user_id = $1 AND sender = $2"
        } else {
            "INSERT INTO disabled_senders (user_id, sender)
             SELECT $1, $2 WHERE EXISTS (SELECT 1 FROM parser_rules WHERE user_id IS NULL AND sender = $2)
             ON CONFLICT DO NOTHING"
        };
        sqlx::query(sql).bind(user_id).bind(&req.sender).execute(&state.pool).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/senders/{sender}", tag = "senders",
    params(("sender" = String, Path,)), security(("bearer" = [])), responses((status = 204)))]
async fn delete_sender(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(sender): Path<String>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("DELETE FROM parser_rules WHERE user_id = $1 AND sender = $2")
        .bind(user_id)
        .bind(sender)
        .execute(&state.pool)
        .await?;
    if done.rows_affected() == 0 { Err(ApiError::not_found()) } else { Ok(StatusCode::NO_CONTENT) }
}

// ============================ unparsed emails ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct UnparsedEmail {
    id: Uuid,
    sender: String,
    subject: String,
    snippet: String,
    received_at: DateTime<Utc>,
    ignored: bool,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UnparsedQuery {
    /// Sertakan yang sudah diabaikan.
    include_ignored: Option<bool>,
    page: Option<i64>,
    per_page: Option<i64>,
}

#[utoipa::path(get, path = "/api/v1/unparsed-emails", tag = "unparsed-emails", params(UnparsedQuery),
    security(("bearer" = [])), responses((status = 200, body = Page<UnparsedEmail>)))]
async fn list_unparsed(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ApiQuery(q): ApiQuery<UnparsedQuery>,
) -> ApiResult<Page<UnparsedEmail>> {
    let (page, per_page) = paging(q.page, q.per_page);
    let all = q.include_ignored.unwrap_or(false);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM unparsed_emails WHERE user_id = $1 AND ($2 OR NOT ignored)",
    )
    .bind(user_id)
    .bind(all)
    .fetch_one(&state.pool)
    .await?;
    let items = sqlx::query_as(
        "SELECT id, sender, subject, snippet, received_at, ignored FROM unparsed_emails
         WHERE user_id = $1 AND ($2 OR NOT ignored)
         ORDER BY received_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(user_id)
    .bind(all)
    .bind(per_page)
    .bind((page - 1) * per_page)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(Page { items, total, page, per_page }))
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateUnparsed {
    ignored: bool,
}

#[utoipa::path(patch, path = "/api/v1/unparsed-emails/{id}", tag = "unparsed-emails",
    params(("id" = Uuid, Path,)), request_body = UpdateUnparsed,
    security(("bearer" = [])), responses((status = 204)))]
async fn update_unparsed(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
    ValidJson(req): ValidJson<UpdateUnparsed>,
) -> Result<StatusCode, ApiError> {
    let done = sqlx::query("UPDATE unparsed_emails SET ignored = $3 WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .bind(req.ignored)
        .execute(&state.pool)
        .await?;
    if done.rows_affected() == 0 { Err(ApiError::not_found()) } else { Ok(StatusCode::NO_CONTENT) }
}

// ============================ reports ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct CategoryTotal {
    category_id: Option<Uuid>,
    name: String,
    color: Option<String>,
    total: i64,
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct DayTotal {
    date: NaiveDate,
    expense: i64,
    income: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Summary {
    month: String,
    total_expense: i64,
    total_income: i64,
    by_category: Vec<CategoryTotal>,
    by_day: Vec<DayTotal>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SummaryQuery {
    /// `YYYY-MM` (Asia/Jakarta). Default: bulan ini.
    month: Option<String>,
    /// Metode pembayaran; kosong = semua.
    provider: Option<String>,
}

/// Batas bulan dihitung di Asia/Jakarta. `$2` = tanggal 1 bulan itu, `$3` = provider (NULL = semua).
/// Hanya IDR yang dijumlahkan (multi-currency di luar cakupan v1).
const IN_MONTH: &str = "t.user_id = $1 AND t.currency = 'IDR' AND ($3::text IS NULL OR t.provider = $3)
    AND t.occurred_at >= ($2::date)::timestamp AT TIME ZONE 'Asia/Jakarta'
    AND t.occurred_at < ($2::date + interval '1 month') AT TIME ZONE 'Asia/Jakarta'";

#[utoipa::path(get, path = "/api/v1/reports/summary", tag = "reports", params(SummaryQuery),
    security(("bearer" = [])), responses((status = 200, body = Summary)))]
async fn summary(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ApiQuery(q): ApiQuery<SummaryQuery>,
) -> ApiResult<Summary> {
    let month = match q.month {
        Some(m) => m,
        None => sqlx::query_scalar("SELECT to_char(now() AT TIME ZONE 'Asia/Jakarta', 'YYYY-MM')")
            .fetch_one(&state.pool)
            .await?,
    };
    let first = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
        .map_err(|_| ApiError::bad_request("month harus berformat YYYY-MM"))?;

    let provider = q.provider.filter(|p| !p.is_empty());

    let mut by_category = QueryBuilder::new(
        "SELECT c.id AS category_id, COALESCE(c.name, 'Tanpa kategori') AS name, c.color,
                SUM(t.amount_minor)::bigint AS total
         FROM transactions t LEFT JOIN categories c ON c.id = t.category_id
         WHERE t.type = 'debit' AND ",
    );
    by_category.push(IN_MONTH).push(" GROUP BY c.id, c.name, c.color ORDER BY total DESC");
    let by_category: Vec<CategoryTotal> = by_category
        .build_query_as()
        .bind(user_id)
        .bind(first)
        .bind(&provider)
        .fetch_all(&state.pool)
        .await?;

    let mut by_day = QueryBuilder::new(
        "SELECT (t.occurred_at AT TIME ZONE 'Asia/Jakarta')::date AS date,
                COALESCE(SUM(t.amount_minor) FILTER (WHERE t.type = 'debit'), 0)::bigint AS expense,
                COALESCE(SUM(t.amount_minor) FILTER (WHERE t.type = 'credit'), 0)::bigint AS income
         FROM transactions t WHERE ",
    );
    by_day.push(IN_MONTH).push(" GROUP BY 1 ORDER BY 1");
    let by_day: Vec<DayTotal> =
        by_day.build_query_as().bind(user_id).bind(first).bind(&provider).fetch_all(&state.pool).await?;

    Ok(Json(Summary {
        month,
        total_expense: by_day.iter().map(|d| d.expense).sum(),
        total_income: by_day.iter().map(|d| d.income).sum(),
        by_category,
        by_day,
    }))
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct MonthTotal {
    /// `YYYY-MM`
    month: String,
    expense: i64,
    income: i64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TrendsQuery {
    /// Jumlah bulan ke belakang termasuk bulan ini (1-24, default 6).
    months: Option<i32>,
}

#[utoipa::path(get, path = "/api/v1/reports/trends", tag = "reports", params(TrendsQuery),
    security(("bearer" = [])), responses((status = 200, body = Vec<MonthTotal>)))]
async fn trends(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    ApiQuery(q): ApiQuery<TrendsQuery>,
) -> ApiResult<Vec<MonthTotal>> {
    // generate_series agar bulan tanpa transaksi tetap muncul sebagai nol.
    let rows = sqlx::query_as(
        "SELECT to_char(m, 'YYYY-MM') AS month,
                COALESCE(SUM(t.amount_minor) FILTER (WHERE t.type = 'debit'), 0)::bigint AS expense,
                COALESCE(SUM(t.amount_minor) FILTER (WHERE t.type = 'credit'), 0)::bigint AS income
         FROM generate_series(
                date_trunc('month', now() AT TIME ZONE 'Asia/Jakarta') - make_interval(months => $2 - 1),
                date_trunc('month', now() AT TIME ZONE 'Asia/Jakarta'),
                interval '1 month') AS m
         LEFT JOIN transactions t
           ON t.user_id = $1 AND t.currency = 'IDR'
          AND date_trunc('month', t.occurred_at AT TIME ZONE 'Asia/Jakarta') = m
         GROUP BY m ORDER BY m",
    )
    .bind(user_id)
    .bind(q.months.unwrap_or(6).clamp(1, 24))
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

// ponytail: tanggal cetak tagihan per kartu ditulis di sini; pindahkan ke tabel + halaman
// Pengaturan bila user perlu mengubahnya dari UI.
const CARD_BILLING_DAYS: [(&str, u32); 1] = [("jenius", 25)];

/// Siklus tagihan yang sedang berjalan: `(awal, tanggal cetak berikutnya)`. `day` 1-28.
/// ponytail: transaksi pada tanggal cetak dihitung ke siklus berikutnya; bank bisa memotong
/// sehari lebih awal/lambat, dan transaksi yang belum di-posting bisa bergeser siklus.
fn billing_period(today: NaiveDate, day: u32) -> Option<(NaiveDate, NaiveDate)> {
    let this_month = today.with_day(day)?;
    let start = if today >= this_month { this_month } else { this_month.checked_sub_months(Months::new(1))? };
    Some((start, start.checked_add_months(Months::new(1))?))
}

#[derive(Serialize, ToSchema)]
pub struct CardBill {
    provider: String,
    /// Awal siklus (inklusif, Asia/Jakarta).
    from: NaiveDate,
    /// Tanggal tagihan terbit; transaksi sebelum tanggal ini dihitung.
    statement_date: NaiveDate,
    /// Debit dikurangi credit (refund) dalam siklus, sen, hanya IDR.
    total: i64,
    count: i64,
    /// Transaksi mata uang asing dalam siklus; tidak ikut `total`.
    foreign_count: i64,
}

/// Estimasi tagihan kartu kredit untuk siklus berjalan, dari notifikasi transaksi.
#[utoipa::path(get, path = "/api/v1/reports/card-bills", tag = "reports", security(("bearer" = [])),
    responses((status = 200, body = Vec<CardBill>)))]
async fn card_bills(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<Vec<CardBill>> {
    let today = Utc::now().with_timezone(&FixedOffset::east_opt(7 * 3600).expect("offset valid")).date_naive();
    let mut bills = Vec::new();
    for (provider, day) in CARD_BILLING_DAYS {
        let (from, statement_date) = billing_period(today, day).expect("day 1-28");
        let (total, count, foreign_count): (i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(CASE type WHEN 'debit' THEN amount_minor ELSE -amount_minor END)
                             FILTER (WHERE currency = 'IDR'), 0)::bigint,
                    count(*), count(*) FILTER (WHERE currency <> 'IDR')
             FROM transactions
             WHERE user_id = $1 AND provider = $2
               AND occurred_at >= ($3::date)::timestamp AT TIME ZONE 'Asia/Jakarta'
               AND occurred_at < ($4::date)::timestamp AT TIME ZONE 'Asia/Jakarta'",
        )
        .bind(user_id)
        .bind(provider)
        .bind(from)
        .bind(statement_date)
        .fetch_one(&state.pool)
        .await?;
        bills.push(CardBill { provider: provider.into(), from, statement_date, total, count, foreign_count });
    }
    Ok(Json(bills))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_period_wraps_month_and_year() {
        let d = |s: &str| s.parse::<NaiveDate>().unwrap();
        assert_eq!(billing_period(d("2026-11-22"), 25), Some((d("2026-10-25"), d("2026-11-25"))));
        assert_eq!(billing_period(d("2026-11-25"), 25), Some((d("2026-11-25"), d("2026-12-25"))));
        assert_eq!(billing_period(d("2027-01-03"), 25), Some((d("2026-12-25"), d("2027-01-25"))));
    }
}

// ============================ sync ============================

#[derive(Serialize, FromRow, ToSchema)]
pub struct SyncStatus {
    /// idle | syncing | error | needs_reauth | disconnected
    status: String,
    last_synced_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    /// true jika backend berjalan dengan MOCK_GMAIL=true.
    mock: bool,
}

#[utoipa::path(get, path = "/api/v1/sync/status", tag = "sync", security(("bearer" = [])),
    responses((status = 200, body = SyncStatus)))]
async fn sync_status(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> ApiResult<SyncStatus> {
    let row = sqlx::query_as(
        "SELECT COALESCE(s.status, 'idle') AS status, s.last_synced_at, s.last_error, $2 AS mock
         FROM users u LEFT JOIN sync_state s ON s.user_id = u.id WHERE u.id = $1",
    )
    .bind(user_id)
    .bind(state.cfg.mock_gmail)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

/// Memicu satu putaran sinkron di latar belakang. Pantau lewat `/sync/status`.
#[utoipa::path(post, path = "/api/v1/sync", tag = "sync", security(("bearer" = [])),
    responses((status = 202), (status = 409, description = "Google terputus")))]
async fn trigger_sync(State(state): State<AppState>, AuthUser(user_id): AuthUser) -> Result<StatusCode, ApiError> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM sync_state WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;
    if status.as_deref() == Some("disconnected") {
        return Err(ApiError::new(StatusCode::CONFLICT, "Google terputus. Login ulang untuk menyambungkan."));
    }
    tokio::spawn(async move { sync::sync_user(&state, user_id).await });
    Ok(StatusCode::ACCEPTED)
}
