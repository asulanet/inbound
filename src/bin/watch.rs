//! An agent acting on a directive.
//!
//! It reads the registry, subscribes once to the operator's whole feed, keeps only the channels
//! its directive names, verifies every offer from the bytes the seller signed, and interrupts its
//! person only when something clears the bar they set.
//!
//! It takes everything the operator carries on purpose. Asking for just its own channels would be
//! cheaper and would tell the operator what its person is shopping for, which is a lead whether or
//! not a name comes with it. Each offer carries its position within its own channel, so the agent
//! can still tell it missed one in a channel it kept without ever naming that channel.

use commonware_codec::DecodeExt;
use commonware_cryptography::Signer;
use commonware_cryptography::ed25519::PrivateKey;
use futures::StreamExt;
use inbound::{Agent, Buyer, Decision, Offer, directive_for};
use serde_json::Value;
use std::collections::HashMap;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// An offer that verified, with the position the operator gave it inside its own channel.
struct Arrival {
    channel_seq: u64,
    offer: Offer,
}

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

    let client = reqwest::Client::new();
    let registry: Value = client
        .get(format!("{base}/registry"))
        .send()
        .await?
        .json()
        .await?;
    let carried = registry
        .get("channels")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let stream_url = registry
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let since_url = registry
        .get("since")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    println!(
        "{DIM}  operator carries {carried} channels; subscribing to all of them, keeping {}{RESET}\n",
        agent.directive.channels.join(", ")
    );

    // Catch up first, then hold the one stream open.
    let history: Vec<Value> = client.get(&since_url).send().await?.json().await?;
    let caught: Vec<Arrival> = history.iter().filter_map(decode).collect();
    let mut next_in_channel: HashMap<String, u64> = HashMap::new();
    let mut seen: Vec<Offer> = Vec::new();
    for arrival in caught {
        keep(&mut seen, &mut next_in_channel, &agent, arrival);
    }
    println!(
        "  {DIM}caught up on {} offer(s) across the feed, {} worth keeping{RESET}\n",
        history.len(),
        seen.len()
    );
    report(&mut agent, &seen, now()?);

    let mut live = Box::pin(arrivals_from(client, stream_url));
    while let Some(arrival) = live.next().await {
        if keep(&mut seen, &mut next_in_channel, &agent, arrival) {
            report(&mut agent, &seen, now()?);
        }
    }
    Ok(())
}

/// Filter on the agent's own side, and notice a gap in a channel it kept. Returns whether the
/// offer was one this agent cares about.
fn keep(
    seen: &mut Vec<Offer>,
    next_in_channel: &mut HashMap<String, u64>,
    agent: &Agent,
    arrival: Arrival,
) -> bool {
    let channel = arrival.offer.body.channel.clone();
    if !agent.directive.channels.contains(&channel) {
        return false;
    }
    if let Some(expected) = next_in_channel.get(&channel)
        && arrival.channel_seq > *expected
    {
        println!(
            "  {YELLOW}missed {} in {channel}{RESET} {DIM}(expected #{expected}, got #{}){RESET}",
            arrival.channel_seq - expected,
            arrival.channel_seq
        );
    }
    next_in_channel.insert(channel, arrival.channel_seq + 1);
    seen.push(arrival.offer);
    true
}

/// The feed's SSE stream, as offers that already verified.
fn arrivals_from(client: reqwest::Client, url: String) -> impl futures::Stream<Item = Arrival> {
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
                if let Some(arrival) = parsed.as_ref().and_then(decode) {
                    yield arrival;
                }
            }
        }
    }
}

/// Decode from the signed bytes, not the readable fields, so a relaying operator cannot edit a
/// rate on the way past. The envelope around them is the operator's own word, so an envelope that
/// disagrees with the signed body about which channel this is gets dropped with it.
fn decode(value: &Value) -> Option<Arrival> {
    let bytes = hex::decode(value.get("bytes")?.as_str()?).ok()?;
    let offer = Offer::decode(bytes.as_slice()).ok()?;
    if !offer.verify() {
        return None;
    }
    if value.get("channel").and_then(Value::as_str) != Some(offer.body.channel.as_str()) {
        return None;
    }
    Some(Arrival {
        channel_seq: value.get("channel_seq")?.as_u64()?,
        offer,
    })
}

fn now() -> Result<u64, std::time::SystemTimeError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

fn report(agent: &mut Agent, seen: &[Offer], now: u64) {
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
