//! Menjalankan parser terhadap semua email di fixtures/emails.json.
//! Fixture yang sama dipakai worker saat MOCK_GMAIL=true.

use chrono::{DateTime, Utc};
use expense_backend::parser::{Email, TxType, load_rules, parse};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Fixture {
    id: String,
    from: String,
    subject: String,
    body: String,
    received_at: DateTime<Utc>,
    /// null = email harus gagal parse.
    expect: Option<Expect>,
}

#[derive(Deserialize)]
struct Expect {
    provider: String,
    amount_minor: i64,
    #[serde(rename = "type")]
    tx_type: TxType,
    merchant: Option<String>,
    occurred_at: DateTime<Utc>,
}

const PROVIDERS: [&str; 9] = [
    "bca", "mandiri", "bri", "jenius", "gopay", "ovo", "dana", "shopeepay", "generic",
];

#[test]
fn all_fixtures_parse_as_expected() {
    let rules = load_rules(include_str!("../config/senders.toml")).expect("senders.toml valid");
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("../fixtures/emails.json")).expect("emails.json valid");

    let mut failures = Vec::new();
    let mut per_provider: HashMap<&str, usize> = HashMap::new();
    let mut unparsed = 0;

    for f in &fixtures {
        let got = parse(
            &rules,
            &Email {
                sender: &f.from,
                subject: &f.subject,
                body: &f.body,
                received_at: f.received_at,
            },
        );
        match (&f.expect, got) {
            (None, None) => unparsed += 1,
            (None, Some(p)) => failures.push(format!("{}: harus gagal parse, dapat {p:?}", f.id)),
            (Some(_), None) => failures.push(format!("{}: tidak ter-parse", f.id)),
            (Some(e), Some(p)) => {
                *per_provider.entry(e.provider.as_str()).or_default() += 1;
                let same = p.provider == e.provider
                    && p.amount_minor == e.amount_minor
                    && p.tx_type == e.tx_type
                    && p.merchant == e.merchant
                    && p.occurred_at == e.occurred_at;
                if !same {
                    failures.push(format!(
                        "{}: dapat {p:?}, harap provider={} amount={} type={:?} merchant={:?} at={}",
                        f.id, e.provider, e.amount_minor, e.tx_type, e.merchant, e.occurred_at
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "{} gagal:\n{}", failures.len(), failures.join("\n"));
    for p in PROVIDERS {
        let n = per_provider.get(p).copied().unwrap_or(0);
        assert!(n >= 10, "provider {p}: hanya {n} sampel, minimal 10");
    }
    assert!(unparsed >= 1, "butuh minimal 1 sampel yang gagal parse");
}
