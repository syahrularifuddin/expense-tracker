//! Rule engine: email notifikasi bank / e-wallet -> transaksi.

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Debit,
    Credit,
}

impl TxType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debit => "debit",
            Self::Credit => "credit",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "debit" => Some(Self::Debit),
            "credit" => Some(Self::Credit),
            _ => None,
        }
    }
}

/// Satu aturan seperti tertulis di `config/senders.toml` / tabel `parser_rules`.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    pub provider: String,
    #[serde(default)]
    pub sender: String,
    pub subject_pattern: Option<String>,
    pub body_regex: String,
    pub default_type: Option<TxType>,
    #[serde(default)]
    pub priority: i32,
    /// false = aturan hanya templat (mis. `generic` untuk sender tambahan user).
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Frasa subjek untuk query pencarian Gmail (`subject:"..."`). Kosong = hanya `from:`.
    pub search_subject: Option<String>,
    /// Nama kategori default untuk transaksi dari aturan ini; menang atas kata kunci merchant.
    pub category: Option<String>,
}

fn enabled_default() -> bool {
    true
}

#[derive(Debug)]
pub struct Rule {
    provider: String,
    sender: String,
    subject: Option<Regex>,
    body: Regex,
    default_type: Option<TxType>,
    priority: i32,
    category: Option<String>,
}

impl Rule {
    /// Pengirim dan subjek cocok: email ini dianggap notifikasi transaksi.
    fn claims(&self, sender_lower: &str, subject: &str) -> bool {
        sender_lower.contains(&self.sender) && self.subject.as_ref().is_none_or(|s| s.is_match(subject))
    }

    pub fn compile(c: RuleConfig) -> Result<Self, BoxError> {
        Ok(Self {
            subject: c.subject_pattern.as_deref().map(Regex::new).transpose()?,
            body: Regex::new(&c.body_regex)?,
            provider: c.provider,
            sender: c.sender.to_lowercase(),
            default_type: c.default_type,
            priority: c.priority,
            category: c.category,
        })
    }
}

/// Kompilasi aturan dan urutkan: priority terbesar dicoba lebih dulu.
pub fn compile_rules(configs: Vec<RuleConfig>) -> Result<Vec<Rule>, BoxError> {
    let mut rules = configs
        .into_iter()
        .map(Rule::compile)
        .collect::<Result<Vec<_>, _>>()?;
    rules.sort_by_key(|r| std::cmp::Reverse(r.priority));
    Ok(rules)
}

#[derive(Deserialize)]
struct RulesFile {
    rule: Vec<RuleConfig>,
}

pub fn load_rule_configs(toml_src: &str) -> Result<Vec<RuleConfig>, BoxError> {
    Ok(toml::from_str::<RulesFile>(toml_src)?.rule)
}

pub fn load_rules(toml_src: &str) -> Result<Vec<Rule>, BoxError> {
    compile_rules(load_rule_configs(toml_src)?)
}

pub struct Email<'a> {
    pub sender: &'a str,
    pub subject: &'a str,
    /// Teks polos (HTML sudah dibuang).
    pub body: &'a str,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub provider: String,
    /// Dalam sen: Rp 12.500,50 -> 1250050.
    pub amount_minor: i64,
    pub tx_type: TxType,
    pub merchant: Option<String>,
    pub occurred_at: DateTime<Utc>,
    /// Kode ISO 4217 dari group `currency`; default IDR ("Rp" juga IDR).
    pub currency: String,
    pub category: Option<String>,
}

/// Ada aturan yang pengirim + subjeknya cocok. Email lain (promo, OTP, info akun)
/// dilewati sepenuhnya, tidak dicatat sebagai gagal parse.
pub fn claims(rules: &[Rule], sender: &str, subject: &str) -> bool {
    let sender = sender.to_lowercase();
    rules.iter().any(|r| r.claims(&sender, subject))
}

/// Aturan pertama (urut priority) yang sender + subject-nya cocok dan
/// menghasilkan nominal > 0 menang. `None` = email masuk `unparsed_emails`.
pub fn parse(rules: &[Rule], email: &Email) -> Option<Parsed> {
    let body = email.body.replace("\r\n", "\n").replace('\u{a0}', " ");
    let sender = email.sender.to_lowercase();
    rules
        .iter()
        .filter(|r| r.claims(&sender, email.subject))
        .find_map(|r| {
            r.body.captures_iter(&body).find_map(|c| {
                let amount_minor = parse_amount(c.name("amount")?.as_str()).filter(|a| *a > 0)?;
                let tx_type = match (c.name("type"), r.default_type) {
                    (Some(t), _) => infer_type(t.as_str()),
                    (None, Some(t)) => t,
                    (None, None) => infer_type(&format!("{} {}", email.subject, body)),
                };
                Some(Parsed {
                    provider: r.provider.clone(),
                    amount_minor,
                    tx_type,
                    merchant: c
                        .name("merchant")
                        .map(|m| m.as_str().split_whitespace().collect::<Vec<_>>().join(" "))
                        .filter(|m| !m.is_empty()),
                    occurred_at: c
                        .name("date")
                        .and_then(|d| parse_date(d.as_str()))
                        .unwrap_or(email.received_at),
                    currency: c
                        .name("currency")
                        .map(|m| m.as_str().trim().to_uppercase())
                        .filter(|c| c.len() == 3 && c.chars().all(|ch| ch.is_ascii_alphabetic()))
                        .unwrap_or_else(|| "IDR".into()),
                    category: r.category.clone(),
                })
            })
        })
}

/// "1.500.000" -> 150000000, "12.500,50" -> 1250050, "50,000.00" -> 5000000.
/// Pemisah terakhir yang diikuti 1-2 digit dianggap desimal; sisanya ribuan.
pub fn parse_amount(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches(['.', ',']);
    let (whole, frac) = match s.rfind(['.', ',']) {
        Some(i) if s.len() - i - 1 <= 2 => (&s[..i], &s[i + 1..]),
        _ => (s, ""),
    };
    let digits: String = whole.chars().filter(char::is_ascii_digit).collect();
    let cents: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().ok()? * 10,
        _ => frac.parse().ok()?,
    };
    digits.parse::<i64>().ok()?.checked_mul(100)?.checked_add(cents)
}

/// Kata kunci penerimaan -> credit, selain itu debit.
pub fn infer_type(text: &str) -> TxType {
    const CREDIT: [&str; 13] = [
        "dana masuk", "menerima", "diterima", "top up", "top-up", "topup", "isi saldo",
        "refund", "pengembalian", "cashback", "kredit", "credit", "received",
    ];
    // "kartu kredit" adalah alat bayar, bukan penerimaan.
    let t = text.to_lowercase().replace("kartu kredit", "");
    if CREDIT.iter().any(|k| t.contains(k)) {
        TxType::Credit
    } else {
        TxType::Debit
    }
}

static ISO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})").expect("static regex"));
static DMY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{1,2})[\s/.-]+([A-Za-z]+|\d{1,2})[\s/.-]+(\d{4})").expect("static regex")
});
static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{1,2})[:.](\d{2})(?:[:.](\d{2}))?").expect("static regex"));

/// Tanggal (+ jam opsional setelahnya) dalam zona Asia/Jakarta (UTC+7, tanpa DST).
/// Format: `21/11/2026 14:30:05`, `21-11-2026 14:30`, `21 Nov 2026, 14:30`,
/// `21 Agustus 2026 pukul 14:30 WIB`, `2026-11-21 14:30:05`. Tanpa jam = 00:00.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let (y, m, d, end): (i32, u32, u32, usize) = if let Some(c) = ISO.captures(s) {
        (c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?, c.get(0)?.end())
    } else {
        let c = DMY.captures(s)?;
        let month = c[2].parse().ok().or_else(|| month_from_name(&c[2]))?;
        (c[3].parse().ok()?, month, c[1].parse().ok()?, c.get(0)?.end())
    };
    let num = |m: Option<regex::Match>| m.and_then(|x| x.as_str().parse::<u32>().ok()).unwrap_or(0);
    let (h, min, sec) = TIME
        .captures(&s[end..])
        .map_or((0, 0, 0), |c| (num(c.get(1)), num(c.get(2)), num(c.get(3))));
    let naive = NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(h, min, sec)?;
    FixedOffset::east_opt(7 * 3600)?
        .from_local_datetime(&naive)
        .single()
        .map(|t| t.with_timezone(&Utc))
}

fn month_from_name(name: &str) -> Option<u32> {
    Some(match name.get(..3)?.to_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "mei" | "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "agu" | "ags" | "agt" | "aug" => 8,
        "sep" => 9,
        "okt" | "oct" => 10,
        "nov" => 11,
        "des" | "dec" => 12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_formats() {
        assert_eq!(parse_amount("1.500.000"), Some(150_000_000));
        assert_eq!(parse_amount("12.500,50"), Some(1_250_050));
        assert_eq!(parse_amount("50,000.00"), Some(5_000_000));
        assert_eq!(parse_amount("28.000,00"), Some(2_800_000));
        assert_eq!(parse_amount("500"), Some(50_000));
        assert_eq!(parse_amount("45.500."), Some(4_550_000)); // titik akhir kalimat
        assert_eq!(parse_amount("7,5"), Some(750));
        assert_eq!(parse_amount("10,000,000.00"), Some(1_000_000_000));
        assert_eq!(parse_amount("1.38"), Some(138));
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount(".,"), None);
    }

    #[test]
    fn date_formats() {
        let utc = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        assert_eq!(parse_date("21/11/2026 14:30:05"), Some(utc("2026-11-21T07:30:05Z")));
        assert_eq!(parse_date("21-11-2026 14:30"), Some(utc("2026-11-21T07:30:00Z")));
        assert_eq!(parse_date("2026-11-21 14:30:05"), Some(utc("2026-11-21T07:30:05Z")));
        assert_eq!(parse_date("5 Agu 2026, 09.05"), Some(utc("2026-08-05T02:05:00Z")));
        assert_eq!(
            parse_date("1 Desember 2026 pukul 00:15 WIB."),
            Some(utc("2026-11-30T17:15:00Z"))
        );
        assert_eq!(parse_date("17 Mei 2026\nJam\n23:59:59 WIB"), Some(utc("2026-05-17T16:59:59Z")));
        assert_eq!(parse_date("3 Okt 2026"), Some(utc("2026-10-02T17:00:00Z")));
        // Format email asli: BCA ("Agt"), Jenius (huruf besar + WIB), Krom ("12.35", "(WIB)").
        assert_eq!(parse_date("19 Agt 2026 08:10:00"), Some(utc("2026-08-19T01:10:00Z")));
        assert_eq!(parse_date("21 NOV 2026 15:25:30 WIB"), Some(utc("2026-11-21T08:25:30Z")));
        assert_eq!(parse_date("21 November 2026, 12.35"), Some(utc("2026-11-21T05:35:00Z")));
        assert_eq!(parse_date("18-11-2026 11:16 (WIB)"), Some(utc("2026-11-18T04:16:00Z")));
        assert_eq!(parse_date("31/02/2026 10:00"), None);
        assert_eq!(parse_date("tidak ada tanggal"), None);
    }

    #[test]
    fn type_inference() {
        assert_eq!(infer_type("Dana Masuk ke rekening"), TxType::Credit);
        assert_eq!(infer_type("Top Up berhasil"), TxType::Credit);
        assert_eq!(infer_type("Pembayaran berhasil"), TxType::Debit);
        assert_eq!(infer_type("Transaksi Kartu Kredit"), TxType::Debit);
    }
}
