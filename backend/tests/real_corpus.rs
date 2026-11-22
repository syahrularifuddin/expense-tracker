//! Ukur parser terhadap email asli yang diunduh ke file lokal (tidak ikut repo).
//!
//! CORPUS=/path/corpus.json cargo test --test real_corpus -- --ignored --nocapture
//!
//! Format corpus: {"<grup>": [{"id","from","subject","internalDate","body"}]}.

use chrono::DateTime;
use expense_backend::parser::{Email, claims, compile_rules, load_rule_configs, parse};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Deserialize)]
struct Mail {
    from: String,
    subject: String,
    #[serde(rename = "internalDate")]
    internal_date: String,
    body: String,
}

#[test]
#[ignore = "butuh CORPUS=<file> berisi email asli"]
fn real_corpus() {
    let path = std::env::var("CORPUS").expect("set CORPUS=<file>");
    let corpus: HashMap<String, Vec<Mail>> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("baca corpus")).expect("corpus JSON");
    let configs = load_rule_configs(include_str!("../config/senders.toml")).expect("senders.toml");
    let rules = compile_rules(configs.into_iter().filter(|c| c.enabled).collect()).expect("regex");

    for (group, mails) in corpus {
        let (mut ok, mut skipped, mut same_as_received) = (0, 0, 0);
        let mut shown = std::collections::HashSet::new();
        let mut failed: BTreeMap<String, (usize, String)> = BTreeMap::new();
        let mut currencies: BTreeMap<String, usize> = BTreeMap::new();
        for m in &mails {
            if !claims(&rules, &m.from, &m.subject) {
                skipped += 1;
                continue;
            }
            let received_at = DateTime::from_timestamp_millis(m.internal_date.parse().unwrap()).unwrap();
            match parse(&rules, &Email { sender: &m.from, subject: &m.subject, body: &m.body, received_at }) {
                Some(p) => {
                    ok += 1;
                    // SHOW=1: satu contoh per (kategori, awal merchant) untuk cek nilai.
                    if std::env::var("SHOW").is_ok() {
                        let key = format!("{:?}/{}", p.category, p.merchant.as_deref().unwrap_or("-").chars().take(10).collect::<String>());
                        if shown.insert(key) && shown.len() <= 40 {
                            println!("  {group}: {:?} {} {} {} | {:?}", p.category, p.amount_minor, p.currency, p.occurred_at, p.merchant);
                        }
                    }
                    if p.occurred_at == received_at {
                        same_as_received += 1;
                    }
                    *currencies.entry(p.currency).or_default() += 1;
                }
                None => {
                    // Kelompokkan kegagalan per baris "jenis" agar mudah dilihat polanya.
                    let lines: Vec<&str> = m.body.lines().collect();
                    let key = lines
                        .iter()
                        .position(|l| matches!(*l, "Transaction Type" | "Transfer Type" | "Status"))
                        .and_then(|i| lines.get(i + 2))
                        .map_or_else(|| m.subject.clone(), |v| v.to_string());
                    let e = failed.entry(key).or_insert((0, m.body.chars().take(900).collect()));
                    e.0 += 1;
                }
            }
        }
        println!("== {group}: {} email, {ok} ter-parse, {skipped} dilewati, {} gagal, mata uang {currencies:?}, \
            tanggal == waktu email {same_as_received}",
            mails.len(), mails.len() - ok - skipped);
        for (key, (n, sample)) in failed {
            println!("-- gagal {n}x: {key}\n{sample}\n");
        }
    }
}
