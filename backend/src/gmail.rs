//! Klien Gmail REST minimal (reqwest).

use crate::{AppState, parser::BoxError, sync::RawEmail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::DateTime;
use regex::Regex;
use reqwest::StatusCode;
use serde_json::Value;
use std::{collections::HashSet, fmt, sync::LazyLock, time::Duration};

const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
// ponytail: sinkron penuh dibatasi 10 halaman x 500 pesan (5000 email); naikkan jika perlu.
const MAX_PAGES: usize = 10;

/// Google menolak refresh/access token: user harus login ulang.
#[derive(Debug)]
pub struct NeedsReauth;

impl fmt::Display for NeedsReauth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("akses Google dicabut atau kedaluwarsa")
    }
}
impl std::error::Error for NeedsReauth {}

pub async fn access_token(state: &AppState, refresh_token: &str) -> Result<String, BoxError> {
    let res = state
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", state.cfg.google_client_id.as_str()),
            ("client_secret", state.cfg.google_client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(reqwest::Error::without_url)?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    if body["error"] == "invalid_grant" {
        return Err(NeedsReauth.into());
    }
    body["access_token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("token endpoint Google membalas {status}").into())
}

/// GET dengan backoff eksponensial (1s .. 60s) pada 429, 5xx, dan 403 kuota/rate limit.
async fn get_json(
    state: &AppState,
    token: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<(StatusCode, Value), BoxError> {
    let mut attempt = 0;
    loop {
        let res = state
            .http
            .get(format!("{API}{path}"))
            .bearer_auth(token)
            .query(query)
            .send()
            .await?;
        let status = res.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(NeedsReauth.into());
        }
        let body = res.json().await.unwrap_or(Value::Null);
        let quota = status == StatusCode::FORBIDDEN
            && body["error"]["message"].as_str().is_some_and(|m| {
                let m = m.to_lowercase();
                m.contains("quota") || m.contains("rate limit")
            });
        if (quota || status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 7 {
            tracing::warn!(%status, attempt, path, "Gmail membatasi, coba lagi");
            tokio::time::sleep(Duration::from_secs((1 << attempt).min(60))).await;
            attempt += 1;
            continue;
        }
        return Ok((status, body));
    }
}

/// Query pencarian: `(from:a subject:"x") OR (from:b) ...` dari pasangan (pengirim, frasa subjek).
pub fn search_query(searches: &[(String, Option<String>)]) -> String {
    searches
        .iter()
        .map(|(from, subject)| match subject {
            Some(s) => format!("(from:{from} subject:\"{}\")", s.replace('"', "")),
            None => format!("(from:{from})"),
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Email baru yang `wanted(from, subject)` + historyId terbaru untuk sinkron berikutnya.
/// Tanpa historyId (atau historyId kedaluwarsa): cari semua email yang cocok `query`.
/// ID di `known` (sudah jadi transaksi) tidak diambil lagi, sehingga sinkron penuh setelah
/// aturan baru ditambahkan hanya memakai kuota untuk email yang belum tercatat.
pub async fn fetch(
    state: &AppState,
    token: &str,
    query: &str,
    wanted: impl Fn(&str, &str) -> bool,
    history_id: Option<&str>,
    known: &HashSet<String>,
) -> Result<(Vec<RawEmail>, Option<String>), BoxError> {
    // historyId diambil sebelum daftar pesan agar tidak ada celah.
    let (status, profile) = get_json(state, token, "/profile", &[]).await?;
    if !status.is_success() {
        return Err(format!("Gmail profile membalas {status}: {}", profile["error"]["message"]).into());
    }
    let new_history = match &profile["historyId"] {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };

    let incremental = match history_id {
        Some(h) => history_ids(state, token, h).await?,
        None => None,
    };
    let from_history = incremental.is_some();
    let mut ids = match incremental {
        Some(ids) => ids,
        None => search_ids(state, token, query).await?,
    };
    let found = ids.len();
    ids.retain(|id| !known.contains(id));

    // Kuota Gmail: 6.000 unit/menit per user; messages.get format=full = 60 unit, jadi maksimal
    // 100 email/menit. Paralel tidak mempercepat (terukur: hanya menambah 403), maka berurutan;
    // batas kuota ditangani backoff di get_json.
    tracing::info!(count = ids.len(), found, from_history, "mengambil pesan Gmail");
    let mut emails = Vec::new();
    for id in ids {
        let path = format!("/messages/{id}");
        // History berisi semua email masuk: cek header dulu (murah), isi penuh hanya jika relevan.
        if from_history {
            let (status, meta) = get_json(
                state,
                token,
                &path,
                &[("format", "metadata"), ("metadataHeaders", "From"), ("metadataHeaders", "Subject")],
            )
            .await?;
            if !status.is_success() || !wanted(&header(&meta["payload"], "From"), &header(&meta["payload"], "Subject")) {
                continue;
            }
        }
        let (status, msg) = get_json(state, token, &path, &[("format", "full")]).await?;
        if status == StatusCode::NOT_FOUND {
            continue; // pesan sudah dihapus
        }
        if !status.is_success() {
            // Jangan dilewati diam-diam: historyId akan maju dan email ini hilang selamanya.
            return Err(format!("Gmail message membalas {status}: {}", msg["error"]["message"]).into());
        }
        if let Some(email) = to_email(&msg).filter(|e| wanted(&e.from, &e.subject)) {
            emails.push(email);
        }
    }
    Ok((emails, new_history))
}

/// `None` = historyId kedaluwarsa (HTTP 404), pemanggil harus sinkron penuh.
async fn history_ids(state: &AppState, token: &str, start: &str) -> Result<Option<Vec<String>>, BoxError> {
    let mut ids = Vec::new();
    let mut page_token = String::new();
    loop {
        let mut query = vec![("startHistoryId", start), ("historyTypes", "messageAdded")];
        if !page_token.is_empty() {
            query.push(("pageToken", &page_token));
        }
        let (status, body) = get_json(state, token, "/history", &query).await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(format!("Gmail history membalas {status}: {}", body["error"]["message"]).into());
        }
        for h in body["history"].as_array().into_iter().flatten() {
            for m in h["messagesAdded"].as_array().into_iter().flatten() {
                if let Some(id) = m["message"]["id"].as_str() {
                    ids.push(id.to_string());
                }
            }
        }
        match body["nextPageToken"].as_str() {
            Some(t) => page_token = t.to_string(),
            None => return Ok(Some(ids)),
        }
    }
}

async fn search_ids(state: &AppState, token: &str, q: &str) -> Result<Vec<String>, BoxError> {
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    let mut page_token = String::new();
    for _ in 0..MAX_PAGES {
        let mut query = vec![("q", q), ("maxResults", "500")];
        if !page_token.is_empty() {
            query.push(("pageToken", &page_token));
        }
        let (status, body) = get_json(state, token, "/messages", &query).await?;
        if !status.is_success() {
            return Err(format!("Gmail messages membalas {status}: {}", body["error"]["message"]).into());
        }
        for m in body["messages"].as_array().into_iter().flatten() {
            if let Some(id) = m["id"].as_str() {
                ids.push(id.to_string());
            }
        }
        match body["nextPageToken"].as_str() {
            Some(t) => page_token = t.to_string(),
            None => break,
        }
    }
    Ok(ids)
}

fn header(payload: &Value, name: &str) -> String {
    payload["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| h["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        .and_then(|h| h["value"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn to_email(msg: &Value) -> Option<RawEmail> {
    let payload = &msg["payload"];
    let header = |name: &str| header(payload, name);
    let decode = |data: &str| {
        URL_SAFE_NO_PAD
            .decode(data.trim_end_matches('='))
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    let body = find_part(payload, "text/plain")
        .and_then(decode)
        .or_else(|| find_part(payload, "text/html").and_then(decode).map(|h| html_to_text(&h)))?;
    Some(RawEmail {
        id: msg["id"].as_str()?.to_string(),
        from: header("From"),
        subject: header("Subject"),
        body,
        received_at: DateTime::from_timestamp_millis(msg["internalDate"].as_str()?.parse().ok()?)?,
    })
}

fn find_part<'a>(part: &'a Value, mime: &str) -> Option<&'a str> {
    if part["mimeType"] == mime
        && let Some(data) = part["body"]["data"].as_str().filter(|d| !d.is_empty())
    {
        return Some(data);
    }
    part["parts"].as_array()?.iter().find_map(|p| find_part(p, mime))
}

static HIDDEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(script|style|head)\b.*?</(script|style|head)>").expect("static regex")
});
static BREAK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(br|/p|/div|/tr|/li|/td|/h[1-6])\b[^>]*>").expect("static regex")
});
static NUMERIC_ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&#(?:[xX]([0-9a-fA-F]+)|([0-9]+));").expect("static regex"));
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]+>").expect("static regex"));

/// HTML -> teks polos: satu baris per blok, tanpa baris kosong.
pub fn html_to_text(html: &str) -> String {
    let s = HIDDEN.replace_all(html, "");
    let s = BREAK.replace_all(&s, "\n");
    let s = TAG.replace_all(&s, "");
    // Entity numerik (&#x2F; -> "/", &#8226; -> "•") sebelum entity bernama; &amp; paling akhir.
    let s = NUMERIC_ENTITY.replace_all(&s, |c: &regex::Captures| {
        let n = match (c.get(1), c.get(2)) {
            (Some(hex), _) => u32::from_str_radix(hex.as_str(), 16).ok(),
            (_, Some(dec)) => dec.as_str().parse().ok(),
            _ => None,
        };
        n.and_then(char::from_u32).map_or_else(|| c[0].to_string(), String::from)
    });
    let s = s
        .replace("&nbsp;", " ")
        .replace("&copy;", "©")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_lines() {
        let html = "<html><head><style>p{color:red}</style></head><body>\
            <p>Pembayaran&nbsp;Berhasil</p><table><tr><td>Nama Merchant</td></tr>\
            <tr><td><b>Kopi &amp; Roti</b></td></tr></table>Total<br/>Rp32.000<br/>\
            <p>11&#x2F;11&#x2F;2026 &#8226; &#47;x &amp;#8226;</p></body></html>";
        assert_eq!(
            html_to_text(html),
            "Pembayaran Berhasil\nNama Merchant\nKopi & Roti\nTotal\nRp32.000\n11/11/2026 • /x &#8226;"
        );
    }

    #[test]
    fn gmail_message_to_email() {
        let msg = serde_json::json!({
            "id": "abc", "internalDate": "1795000000000",
            "payload": {
                "mimeType": "multipart/alternative",
                "headers": [{"name": "From", "value": "OVO <no-reply@ovo.id>"},
                            {"name": "subject", "value": "Pembayaran Berhasil"}],
                "parts": [{"mimeType": "text/html",
                           "body": {"data": URL_SAFE_NO_PAD.encode("<p>Total</p><p>Rp1.000</p>")}}]
            }
        });
        let email = to_email(&msg).unwrap();
        assert_eq!(email.from, "OVO <no-reply@ovo.id>");
        assert_eq!(email.subject, "Pembayaran Berhasil");
        assert_eq!(email.body, "Total\nRp1.000");
    }
}
