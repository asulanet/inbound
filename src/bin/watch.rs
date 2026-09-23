//! An agent acting on a directive.
//!
//! It reads the registry to find the channels its directive names, subscribes to each, verifies
//! every offer from the bytes the seller signed, and interrupts its person only when something
//! clears the bar they set.

use commonware_codec::DecodeExt;
use commonware_cryptography::Signer;
use commonware_cryptography::ed25519::PrivateKey;
use futures::StreamExt;
use futures::stream::{self, BoxStream};
use inbound::{Agent, Buyer, Decision, Offer, directive_for};
use serde_json::Value;
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
    let mut agent = Agent::new(buyer(&who), directive_for(&who));

    println!("\n{BOLD}{who}{RESET}{DIM} told their agent:{RESET}");
    println!("  {DIM}watching for{RESET} {}", agent.directive.about);
    println!(
        "  {DIM}would spend{RESET} ${}   {DIM}wake me above{RESET} ${:.2}",
        agent.directive.budget / 100,
        agent.directive.worth_waking_me as f64 / 100.0
    );

    // Find the channels the directive names, and how to reach each one.
    let client = reqwest::Client::new();
    let registry: Value = client
        .get(format!("{base}/registry"))
        .send()
        .await?
        .json()
        .await?;
    let listed = registry
        .get("channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut wanted = Vec::new();
    for entry in &listed {
        let Some(name) = entry.get("channel").and_then(Value::as_str) else {
            continue;
        };
        if agent.directive.channels.iter().any(|want| want == name) {
            let stream = entry.get("stream").and_then(Value::as_str).unwrap_or("");
            let since = entry.get("since").and_then(Value::as_str).unwrap_or("");
            wanted.push((name.to_string(), stream.to_string(), since.to_string()));
        }
    }
    println!(
        "{DIM}  registry lists {} channels, {} of them wanted{RESET}\n",
        listed.len(),
        wanted.len()
    );

    // Catch up on each, then hold every stream open at once.
    let mut seen: Vec<Offer> = Vec::new();
    let mut streams: Vec<BoxStream<'static, Offer>> = Vec::new();
    for (name, stream_url, since_url) in wanted {
        let history: Vec<Value> = client.get(&since_url).send().await?.json().await?;
        let caught: Vec<Offer> = history.iter().filter_map(decode).collect();
        println!(
            "  {DIM}subscribed to {name}, caught up on {} offer(s){RESET}",
            caught.len()
        );
        seen.extend(caught);
        streams.push(Box::pin(offers_from(client.clone(), stream_url)));
    }
    println!();

    report(&mut agent, &seen);

    let mut merged = stream::select_all(streams);
    while let Some(offer) = merged.next().await {
        seen.push(offer);
        report(&mut agent, &seen);
    }
    Ok(())
}

/// One channel's SSE stream, as offers that already verified.
fn offers_from(client: reqwest::Client, url: String) -> impl futures::Stream<Item = Offer> {
    async_stream::stream! {
        let Ok(response) = client.get(&url).send().await else { return };
        let mut bytes = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(Ok(chunk)) = bytes.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find("\n\n") {
                let frame: String = buffer.drain(..end + 2).collect();
                let parsed = frame
                    .lines()
                    .find_map(|line| line.strip_prefix("data: "))
                    .and_then(|data| serde_json::from_str::<Value>(data).ok());
                if let Some(offer) = parsed.as_ref().and_then(decode) {
                    yield offer;
                }
            }
        }
    }
}

/// Decode from the signed bytes, not the readable fields, so a relaying operator cannot edit a
/// rate on the way past.
fn decode(value: &Value) -> Option<Offer> {
    let bytes = hex::decode(value.get("bytes")?.as_str()?).ok()?;
    let offer = Offer::decode(bytes.as_slice()).ok()?;
    offer.verify().then_some(offer)
}

fn report(agent: &mut Agent, seen: &[Offer]) {
    let now = seen
        .iter()
        .map(|offer| offer.body.expires)
        .min()
        .unwrap_or(0)
        .saturating_sub(86_400);
    match agent.consider(seen, now) {
        Decision::Surface { seller, worth } => println!(
            "  {BOLD}worth telling them{RESET} → {seller} at {GREEN}${:.2}{RESET} {DIM}(over their ${:.2} bar){RESET}",
            worth as f64 / 100.0,
            agent.directive.worth_waking_me as f64 / 100.0
        ),
        Decision::Hold { seller, worth } => println!(
            "  {DIM}holding {seller} at ${:.2}, not worth interrupting for{RESET}",
            worth as f64 / 100.0
        ),
        Decision::Ignore => println!(
            "  {DIM}nothing better. {} passed over.{RESET}",
            agent.ignored
        ),
    }
}

fn buyer(who: &str) -> Buyer {
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
