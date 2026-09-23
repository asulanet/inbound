//! An agent that connects to a channel it does not run, and gets told.
//!
//! It verifies every offer from the bytes the seller signed, ranks what it has against its own
//! user's situation, and speaks only when its answer changes.

use commonware_codec::DecodeExt;
use commonware_cryptography::Signer;
use commonware_cryptography::ed25519::PrivateKey;
use futures::StreamExt;
use inbound::{Buyer, Offer, name_of, rank};
use serde_json::Value;
use std::collections::HashMap;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";
const CHANNEL: &str = "cashback/us/sportswear";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:7402".into());
    let who = std::env::args().nth(2).unwrap_or_else(|| "maya".into());
    let buyer = person(&who);

    println!(
        "\n{BOLD}{}{RESET}{DIM} · ${} basket · {}{RESET}",
        buyer.name,
        buyer.basket / 100,
        if buyer.spent_with.is_empty() {
            "new to every shop here"
        } else {
            "already shops at runfast"
        }
    );

    let client = reqwest::Client::new();
    let mut seen: Vec<Offer> = Vec::new();

    // Whatever was posted before we connected.
    let history: Vec<Value> = client
        .get(format!("{base}/channels/{CHANNEL}/offers?since=0"))
        .send()
        .await?
        .json()
        .await?;
    seen.extend(history.iter().filter_map(decode));
    println!(
        "{DIM}connected · caught up on {} offer(s){RESET}\n",
        seen.len()
    );

    let mut best: Option<(String, u64)> = None;
    let mut passed = 0u32;
    report(&mut best, &mut passed, &seen, &buyer);

    // Then hold the stream open and be told.
    let mut stream = client
        .get(format!("{base}/channels/{CHANNEL}/stream"))
        .send()
        .await?
        .bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(end) = buffer.find("\n\n") {
            let frame: String = buffer.drain(..end + 2).collect();
            let Some(value) = frame
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .and_then(|data| serde_json::from_str::<Value>(data).ok())
            else {
                continue;
            };
            match decode(&value) {
                Some(offer) => {
                    seen.push(offer);
                    report(&mut best, &mut passed, &seen, &buyer);
                }
                None => println!("  {DIM}an offer arrived that did not verify. dropped.{RESET}"),
            }
        }
    }
    Ok(())
}

/// Decode from the signed bytes, not the readable fields, so a relaying operator cannot edit a
/// rate on the way past.
fn decode(value: &Value) -> Option<Offer> {
    let bytes = hex::decode(value.get("bytes")?.as_str()?).ok()?;
    let offer = Offer::decode(bytes.as_slice()).ok()?;
    offer.verify().then_some(offer)
}

fn report(best: &mut Option<(String, u64)>, passed: &mut u32, seen: &[Offer], buyer: &Buyer) {
    let now = seen
        .iter()
        .map(|offer| offer.body.expires)
        .min()
        .unwrap_or(0);
    let Some(top) = rank(seen, buyer, now.saturating_sub(86_400))
        .into_iter()
        .next()
    else {
        *passed += 1;
        return;
    };
    if best.as_ref().is_some_and(|(_, worth)| top.worth <= *worth) {
        *passed += 1;
        println!("  {DIM}nothing better. {passed} passed over so far.{RESET}");
        return;
    }
    let seller = name_of(&top.offer.body.seller);
    match best.replace((seller.clone(), top.worth)) {
        Some((was, worth)) => println!(
            "  {BOLD}better{RESET} → {seller} at {GREEN}${:.2}{RESET} {DIM}(was {was} at ${:.2}){RESET}",
            top.worth as f64 / 100.0,
            worth as f64 / 100.0
        ),
        None => println!(
            "  holding {seller} at {GREEN}${:.2}{RESET}",
            top.worth as f64 / 100.0
        ),
    }
}

fn person(who: &str) -> Buyer {
    let runfast = PrivateKey::from_seed(10).public_key();
    match who {
        "sam" => Buyer {
            name: "sam".into(),
            basket: 5_000,
            balance: 40_000,
            spent_with: HashMap::from([(runfast, 22_000)]),
        },
        _ => Buyer {
            name: "maya".into(),
            basket: 12_000,
            balance: 250_000,
            spent_with: HashMap::new(),
        },
    }
}
