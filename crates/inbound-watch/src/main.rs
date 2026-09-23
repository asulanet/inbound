//! An agent that connects to a channel it does not run and gets pushed to.
//!
//! It holds the stream open, verifies every offer's signature itself from the canonical bytes,
//! ranks what it has seen against its own user's situation, and speaks only when its answer
//! changes. It never asks the operator for anything.

use commonware_codec::DecodeExt;
use commonware_cryptography::Signer;
use futures::StreamExt;
use inbound_agent::{Buyer, rank};
use inbound_types::{Offer, PrivateKey, PublicKey};
use std::collections::HashMap;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:7402".into());
    let who = std::env::args().nth(2).unwrap_or_else(|| "maya".into());

    // Two different people, so two different answers from the same feed.
    let (buyer, description) = match who.as_str() {
        "sam" => (
            Buyer {
                root: PrivateKey::from_seed(43),
                credential_secret: [9u8; 32],
                available_balance: 40_000,
                spend_by_seller: HashMap::from([(PrivateKey::from_seed(10).public_key(), 22_000)]),
                typical_basket: 5_000,
            },
            "$50 basket, already shops at runfast",
        ),
        _ => (
            Buyer {
                root: PrivateKey::from_seed(42),
                credential_secret: [7u8; 32],
                available_balance: 250_000,
                spend_by_seller: HashMap::new(),
                typical_basket: 12_000,
            },
            "$120 basket, new to every shop here",
        ),
    };

    let channel = "cashback/us/sportswear";
    println!("\n{BOLD}{who}{RESET}{DIM}'s agent · {description}{RESET}");

    // Catch up on anything posted before we connected, then hold the stream open.
    let client = reqwest::Client::new();
    let mut seen: Vec<Offer> = Vec::new();
    let history: Vec<serde_json::Value> = client
        .get(format!("{base}/channels/{channel}/offers?since=0"))
        .send()
        .await?
        .json()
        .await?;
    for item in &history {
        if let Some(offer) = decode(item) {
            seen.push(offer);
        }
    }
    println!(
        "{DIM}connected to {base}/channels/{channel} · caught up on {} offer(s){RESET}\n",
        seen.len()
    );

    let mut best: Option<(String, u64)> = None;
    let mut ignored = 0u32;
    announce(&mut best, &seen, &buyer, &mut ignored, true);

    let response = client
        .get(format!("{base}/channels/{channel}/stream"))
        .send()
        .await?;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(index) = buffer.find("\n\n") {
            let frame: String = buffer.drain(..index + 2).collect();
            let Some(payload) = frame
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .and_then(|data| serde_json::from_str::<serde_json::Value>(data).ok())
            else {
                continue;
            };
            let Some(offer) = decode(&payload) else {
                println!("  {DIM}pushed an offer that failed verification, dropped{RESET}");
                continue;
            };
            seen.push(offer);
            announce(&mut best, &seen, &buyer, &mut ignored, false);
        }
    }
    Ok(())
}

/// Verify from the canonical bytes the seller signed, not from the readable JSON. The operator
/// could have rewritten the pretty fields; it cannot forge the signature.
fn decode(value: &serde_json::Value) -> Option<Offer> {
    let bytes = hex::decode(value.get("bytes")?.as_str()?).ok()?;
    let offer = Offer::decode(bytes.as_slice()).ok()?;
    offer.verify().ok()?;
    Some(offer)
}

fn announce(
    best: &mut Option<(String, u64)>,
    seen: &[Offer],
    buyer: &Buyer,
    ignored: &mut u32,
    quiet_if_same: bool,
) {
    let now = seen
        .iter()
        .map(|offer| offer.body.valid_from)
        .max()
        .unwrap_or(0);
    let ranked = rank(seen.to_vec(), buyer, now, 99);
    let Some(top) = ranked.first() else {
        *ignored += 1;
        return;
    };
    let improved = best
        .as_ref()
        .is_none_or(|(_, cents)| top.expected_reward > *cents);
    if !improved {
        *ignored += 1;
        if !quiet_if_same {
            println!(
                "  {DIM}new offer, worse than what I'm holding. {} ignored so far{RESET}",
                ignored
            );
        }
        return;
    }
    let seller = short(&top.offer.body.seller);
    let previous = best.replace((seller.clone(), top.expected_reward));
    match previous {
        Some((was_seller, was)) => println!(
            "  {BOLD}better{RESET} → {seller} at {GREEN}${:.2}{RESET} {DIM}(was {was_seller} at ${:.2}){RESET}",
            top.expected_reward as f64 / 100.0,
            was as f64 / 100.0
        ),
        None => println!(
            "  holding {seller} at {GREEN}${:.2}{RESET}",
            top.expected_reward as f64 / 100.0
        ),
    }
}

fn short(key: &PublicKey) -> String {
    let names = [
        ("runfast", 10u64),
        ("trailco", 11),
        ("peakform", 12),
        ("cadence", 13),
    ];
    for (name, seed) in names {
        if PrivateKey::from_seed(seed).public_key() == *key {
            return name.to_string();
        }
    }
    format!("{key}").chars().take(8).collect()
}
