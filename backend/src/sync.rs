//! Worker sinkronisasi: ambil email (Gmail atau fixture), parse, simpan transaksi.

use crate::{
    AppState, auth, gmail,
    parser::{self, BoxError, Email, RuleConfig},
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

const POLL_SECS: u64 = 300;
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/emails.json");

#[derive(Debug, Deserialize)]
pub struct RawEmail {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub received_at: DateTime<Utc>,
}

/// Ganti aturan global di DB dengan isi `config/senders.toml`.
pub async fn seed_global_rules(pool: &PgPool, configs: &[RuleConfig]) -> Result<(), BoxError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM parser_rules WHERE user_id IS NULL")
        .execute(&mut *tx)
        .await?;
    for c in configs {
        sqlx::query(
            "INSERT INTO parser_rules (provider, sender, subject_pattern, body_regex, default_type, priority)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&c.provider)
        .bind(c.sender.to_lowercase())
        .bind(&c.subject_pattern)
        .bind(&c.body_regex)
        .bind(c.default_type.map(|t| t.as_str()))
        .bind(c.priority)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Polling tiap 5 menit per user, dengan jitter antar user.
pub async fn worker(state: AppState) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(POLL_SECS));
    loop {
        tick.tick().await;
        if let Err(e) = sqlx::query("DELETE FROM sessions WHERE expires_at < now()")
            .execute(&state.pool)
            .await
        {
            tracing::warn!(error = %e, "hapus sesi kedaluwarsa gagal");
        }
        let users: Vec<Uuid> = sqlx::query_scalar(
            "SELECT u.id FROM users u LEFT JOIN sync_state s ON s.user_id = u.id
             WHERE COALESCE(s.status, 'idle') NOT IN ('needs_reauth', 'disconnected')
               AND ($1 OR EXISTS (SELECT 1 FROM google_tokens g WHERE g.user_id = u.id))",
        )
        .bind(state.cfg.mock_gmail)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        for user_id in users {
            let jitter = rand::random_range(0..10_000);
            tokio::time::sleep(std::time::Duration::from_millis(jitter)).await;
            sync_user(&state, user_id).await;
        }
    }
}

/// Satu putaran sinkron untuk satu user. Tidak melakukan apa-apa jika putaran
/// lain untuk user yang sama masih berjalan. Hasil dicatat di `sync_state`.
pub async fn sync_user(state: &AppState, user_id: Uuid) {
    let begun: Result<Option<Option<String>>, _> = sqlx::query_scalar(
        "INSERT INTO sync_state (user_id, status) VALUES ($1, 'syncing')
         ON CONFLICT (user_id) DO UPDATE SET status = 'syncing'
         WHERE sync_state.status <> 'syncing'
         RETURNING history_id",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await;
    let history_id = match begun {
        Ok(Some(h)) => h,
        Ok(None) => return,
        Err(e) => return tracing::error!(error = %e, "mulai sinkron gagal"),
    };

    let (status, new_history, error) = match run(state, user_id, history_id.as_deref()).await {
        Ok((inserted, new_history)) => {
            tracing::info!(%user_id, inserted, "sinkron selesai");
            ("idle", new_history.or(history_id), None)
        }
        Err(e) if e.downcast_ref::<gmail::NeedsReauth>().is_some() => {
            ("needs_reauth", None, Some(e.to_string()))
        }
        Err(e) => {
            tracing::warn!(%user_id, error = %e, "sinkron gagal");
            ("error", history_id, Some(e.to_string()))
        }
    };
    let done = sqlx::query(
        "UPDATE sync_state SET status = $2, history_id = $3, last_error = $4,
                last_synced_at = CASE WHEN $2 = 'idle' THEN now() ELSE last_synced_at END
         WHERE user_id = $1 AND status = 'syncing'",
    )
    .bind(user_id)
    .bind(status)
    .bind(new_history)
    .bind(error)
    .execute(&state.pool)
    .await;
    if let Err(e) = done {
        tracing::error!(error = %e, "simpan status sinkron gagal");
    }
}

async fn run(
    state: &AppState,
    user_id: Uuid,
    history_id: Option<&str>,
) -> Result<(usize, Option<String>), BoxError> {
    let configs = load_rule_configs(&state.pool, user_id).await?;
    let disabled: Vec<String> =
        sqlx::query_scalar("SELECT sender FROM disabled_senders WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?;

    let (mut emails, new_history, shift) = if state.cfg.mock_gmail {
        let emails: Vec<RawEmail> = serde_json::from_str(&std::fs::read_to_string(FIXTURES)?)?;
        // Tanggal fixture statis; geser (hari utuh) agar yang terbaru jatuh kemarin
        // sehingga dashboard "bulan ini" terisi.
        let latest = emails.iter().map(|e| e.received_at).max().unwrap_or_else(Utc::now);
        let shift = Duration::days((Utc::now() - latest).num_days() - 1);
        (emails, None, shift)
    } else {
        let refresh = auth::gmail_refresh_token(state, user_id)
            .await?
            .ok_or(gmail::NeedsReauth)?;
        let token = gmail::access_token(state, &refresh).await?;
        let mut senders: Vec<String> =
            configs.iter().map(|c| c.sender.clone()).filter(|s| !s.is_empty()).collect();
        senders.sort();
        senders.dedup();
        let (emails, new_history) = gmail::fetch(state, &token, &senders, history_id).await?;
        (emails, new_history, Duration::zero())
    };
    emails.retain(|e| {
        let from = e.from.to_lowercase();
        !disabled.iter().any(|s| from.contains(s.as_str()))
    });

    let rules = parser::compile_rules(configs)?;
    let mut inserted = 0;
    for e in &emails {
        let received_at = e.received_at + shift;
        let parsed = parser::parse(
            &rules,
            &Email { sender: &e.from, subject: &e.subject, body: &e.body, received_at },
        );
        match parsed {
            Some(p) => {
                // Tanggal dari body ikut digeser; fallback received_at sudah tergeser.
                let occurred_at =
                    if p.occurred_at == received_at { received_at } else { p.occurred_at + shift };
                let done = sqlx::query(
                    "INSERT INTO transactions (user_id, occurred_at, amount_minor, type, merchant,
                                               category_id, source, gmail_message_id, raw_subject)
                     VALUES ($1, $2, $3, $4, $5,
                             (SELECT category_id FROM category_rules
                              WHERE (user_id IS NULL OR user_id = $1) AND $5 ILIKE '%' || keyword || '%'
                              ORDER BY user_id NULLS LAST, length(keyword) DESC LIMIT 1),
                             'gmail', $6, $7)
                     ON CONFLICT (user_id, gmail_message_id) DO NOTHING",
                )
                .bind(user_id)
                .bind(occurred_at)
                .bind(p.amount_minor)
                .bind(p.tx_type.as_str())
                .bind(&p.merchant)
                .bind(&e.id)
                .bind(&e.subject)
                .execute(&state.pool)
                .await?;
                inserted += done.rows_affected() as usize;
            }
            None => {
                // Hanya cuplikan yang disimpan, bukan body penuh.
                let snippet: String = e
                    .body
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(200)
                    .collect();
                sqlx::query(
                    "INSERT INTO unparsed_emails (user_id, gmail_message_id, sender, subject, snippet, received_at)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (user_id, gmail_message_id) DO NOTHING",
                )
                .bind(user_id)
                .bind(&e.id)
                .bind(&e.from)
                .bind(&e.subject)
                .bind(snippet)
                .bind(received_at)
                .execute(&state.pool)
                .await?;
            }
        }
    }
    Ok((inserted, new_history))
}

/// Aturan aktif untuk user: global + miliknya, tanpa sender yang ia matikan.
async fn load_rule_configs(pool: &PgPool, user_id: Uuid) -> Result<Vec<RuleConfig>, BoxError> {
    type Row = (String, String, Option<String>, String, Option<String>, i32);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT provider, sender, subject_pattern, body_regex, default_type, priority
         FROM parser_rules r
         WHERE enabled AND (user_id IS NULL OR user_id = $1)
           AND NOT EXISTS (SELECT 1 FROM disabled_senders d
                           WHERE d.user_id = $1 AND d.sender = r.sender)
         ORDER BY user_id NULLS LAST, id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(provider, sender, subject_pattern, body_regex, default_type, priority)| RuleConfig {
            provider,
            sender,
            subject_pattern,
            body_regex,
            default_type: default_type.as_deref().and_then(parser::TxType::from_db),
            priority,
        })
        .collect())
}
