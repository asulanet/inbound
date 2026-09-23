//! Channels anything can connect to.
//!
//! `/registry` says what exists and how to reach it. Each channel serves its log over plain HTTP
//! and pushes new offers over SSE. `/mcp` answers the same thing to a connector-capable agent.
//! Fictional sellers post on a timer.

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::{Router, http::StatusCode};
use commonware_codec::Encode;
use commonware_runtime::{Runner, Supervisor, tokio as cw};
use futures::Stream;
use inbound::{CHANNELS, Channel, Offer, name_of, next_seller, sellers, write_offer};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const EPOCH: u64 = 1_790_812_800;

struct Operator {
    base: String,
    channels: HashMap<String, Mutex<Channel<cw::Context>>>,
}

type Shared = Arc<Operator>;

/// What an agent receives. `bytes` is the canonical encoding the seller signed, so an agent can
/// check the signature itself rather than trusting these readable fields.
#[derive(Serialize)]
struct Wire {
    position: u64,
    seller: String,
    channel: String,
    reward_bps: u16,
    best_bps: u16,
    min_spend: u64,
    expires: u64,
    bytes: String,
}

impl Wire {
    fn of(position: u64, offer: &Offer) -> Self {
        Self {
            position,
            seller: name_of(&offer.body.seller),
            channel: offer.body.channel.clone(),
            reward_bps: offer.body.reward.basis_points,
            best_bps: offer
                .body
                .better
                .iter()
                .map(|terms| terms.reward.basis_points)
                .max()
                .unwrap_or(offer.body.reward.basis_points),
            min_spend: offer.body.min_spend,
            expires: offer.body.expires,
            bytes: hex::encode(offer.encode()),
        }
    }
}

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .or_else(|| std::env::args().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(7402);
    let every: u64 = std::env::var("EVERY_SECONDS")
        .ok()
        .or_else(|| std::env::args().nth(2))
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    // Somewhere stable to keep the logs, so a restart does not lose the channels. The runtime
    // otherwise picks a fresh temp directory each boot.
    let config = cw::Config::default().with_storage_directory(
        std::env::var("STORAGE_DIR").unwrap_or_else(|_| "./storage".into()),
    );
    cw::Runner::new(config).start(|context| async move {
        if let Err(error) = run(context, port, every).await {
            eprintln!("server failed: {error}");
        }
    });
}

async fn run(
    context: cw::Context,
    port: u16,
    every: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("PUBLIC_URL").unwrap_or_else(|_| format!("http://127.0.0.1:{port}"));
    let mut channels = HashMap::new();
    for name in CHANNELS {
        channels.insert(
            name.to_string(),
            Mutex::new(Channel::open(context.child("channel"), name).await?),
        );
    }
    let operator: Shared = Arc::new(Operator {
        base: host.clone(),
        channels,
    });

    let posting = operator.clone();
    tokio::spawn(async move {
        let shops = sellers();
        let mut rng = ChaCha20Rng::seed_from_u64(2_026);
        let mut nonce = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(every)).await;
            nonce += 1;
            let seller = &shops[next_seller(&mut rng, &shops)];
            let offer = write_offer(&mut rng, seller, EPOCH + nonce * 86_400, nonce);
            let Some(channel) = posting.channels.get(seller.channel) else {
                continue;
            };
            let mut channel = channel.lock().await;
            match channel.post(&offer).await {
                Ok(position) => println!(
                    "  {} #{position} {} → {} listening",
                    seller.channel,
                    seller.name,
                    channel.subscribers()
                ),
                Err(error) => eprintln!("  refused: {error}"),
            }
        }
    });

    let app = Router::new()
        .route("/", get(registry))
        .route("/registry", get(registry))
        .route("/channels/{*rest}", get(channel_route))
        .route("/mcp", get(registry).post(mcp))
        .with_state(operator);

    println!("\ninbound · {} channels\n", CHANNELS.len());
    println!("  what exists   curl -s {host}/registry");
    for name in CHANNELS {
        println!("  be told       curl -N {host}/channels/{name}/stream");
    }
    println!("  or by mcp     {host}/mcp");
    println!("\n  an offer lands every {every}s. whoever is listening is told.\n");

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// How an agent finds anything. One URL in, everything it needs to subscribe out.
async fn registry(State(operator): State<Shared>) -> impl IntoResponse {
    let mut listed = Vec::new();
    for name in CHANNELS {
        let Some(channel) = operator.channels.get(*name) else {
            continue;
        };
        let channel = channel.lock().await;
        listed.push(json!({
            "channel": name,
            "offers": channel.tip(),
            "listening": channel.subscribers(),
            "stream": format!("{}/channels/{name}/stream", operator.base),
            "since": format!("{}/channels/{name}/offers?since=0", operator.base),
        }));
    }
    Json(json!({
        "operator": "inbound demo",
        "mcp": format!("{}/mcp", operator.base),
        "channels": listed,
    }))
}

#[derive(Deserialize)]
struct Since {
    since: Option<u64>,
}

async fn channel_route(
    Path(rest): Path<String>,
    Query(since): Query<Since>,
    State(operator): State<Shared>,
) -> Response {
    let (name, action) = match rest.rsplit_once('/') {
        Some((name, tail @ ("stream" | "offers"))) => (name.to_string(), tail),
        _ => (rest.clone(), ""),
    };
    if !operator.channels.contains_key(&name) {
        return (StatusCode::NOT_FOUND, "no such channel").into_response();
    }
    match action {
        "stream" => stream(operator, name).await.into_response(),
        "offers" => Json(recent(&operator, &name, since.since.unwrap_or(0)).await).into_response(),
        _ => registry(State(operator)).await.into_response(),
    }
}

/// The push path. An agent holds this open and is told; it never asks.
async fn stream(
    operator: Shared,
    name: String,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let subscription = match operator.channels.get(&name) {
        Some(channel) => Some(channel.lock().await.subscribe()),
        None => None,
    };
    Sse::new(async_stream::stream! {
        let Some(mut subscription) = subscription else { return };
        while let Some((position, offer)) = subscription.next().await {
            if let Ok(json) = serde_json::to_string(&Wire::of(position, &offer)) {
                yield Ok(Event::default().event("offer").data(json));
            }
        }
    })
    .keep_alive(KeepAlive::default())
}

async fn recent(operator: &Shared, name: &str, since: u64) -> Vec<Wire> {
    let Some(channel) = operator.channels.get(name) else {
        return Vec::new();
    };
    let channel = channel.lock().await;
    channel
        .since(since)
        .await
        .map(|offers| {
            offers
                .iter()
                .map(|(position, offer)| Wire::of(*position, offer))
                .collect()
        })
        .unwrap_or_default()
}

/// Enough MCP for a connector to attach: tools to look, a resource subscription to be told.
async fn mcp(State(operator): State<Shared>, body: String) -> Response {
    let Ok(request) = serde_json::from_str::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(id) = request.get("id").cloned() else {
        return StatusCode::ACCEPTED.into_response();
    };
    let wanted = request
        .pointer("/params/arguments/channel")
        .and_then(Value::as_str)
        .unwrap_or(CHANNELS[0])
        .to_string();

    let result = match request.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {}, "resources": { "subscribe": true } },
            "serverInfo": { "name": "inbound", "version": "0.0.1" }
        }),
        "tools/list" => json!({ "tools": [
            { "name": "list_channels",
              "description": "Every channel this operator runs, and how to subscribe to each.",
              "inputSchema": { "type": "object", "properties": {} } },
            { "name": "list_offers",
              "description": "Offers in a channel from a cursor onward. Catch-up; live ones arrive on the subscription.",
              "inputSchema": { "type": "object", "properties": {
                  "channel": { "type": "string" },
                  "since": { "type": "integer" } } } }
        ]}),
        "resources/list" => json!({ "resources": CHANNELS.iter().map(|name| json!({
            "uri": format!("inbound://{name}"),
            "name": name,
            "mimeType": "application/json"
        })).collect::<Vec<_>>() }),
        "tools/call" => {
            let tool = request
                .pointer("/params/name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let text = if tool == "list_channels" {
                serde_json::to_string_pretty(&registry_value(&operator).await).unwrap_or_default()
            } else {
                let since = request
                    .pointer("/params/arguments/since")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                serde_json::to_string_pretty(&recent(&operator, &wanted, since).await)
                    .unwrap_or_default()
            };
            json!({ "content": [{ "type": "text", "text": text }], "isError": false })
        }
        "resources/read" => {
            let text = serde_json::to_string_pretty(&recent(&operator, &wanted, 0).await)
                .unwrap_or_default();
            json!({ "contents": [{ "uri": format!("inbound://{wanted}"), "text": text }] })
        }
        "subscriptions/listen" => {
            return mcp_stream(operator, id, wanted).await.into_response();
        }
        _ => {
            return Json(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "method not found" }
            }))
            .into_response();
        }
    };
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

async fn registry_value(operator: &Shared) -> Value {
    let mut listed = Vec::new();
    for name in CHANNELS {
        let Some(channel) = operator.channels.get(*name) else {
            continue;
        };
        let channel = channel.lock().await;
        listed.push(json!({
            "channel": name,
            "offers": channel.tip(),
            "stream": format!("{}/channels/{name}/stream", operator.base),
        }));
    }
    json!({ "channels": listed })
}

async fn mcp_stream(
    operator: Shared,
    id: Value,
    name: String,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let subscription = match operator.channels.get(&name) {
        Some(channel) => Some(channel.lock().await.subscribe()),
        None => None,
    };
    let uri = format!("inbound://{name}");
    Sse::new(async_stream::stream! {
        yield Ok(Event::default().data(json!({
            "jsonrpc": "2.0",
            "method": "notifications/subscriptions/acknowledged",
            "params": { "_meta": { "io.modelcontextprotocol/subscriptionId": id } }
        }).to_string()));
        let Some(mut subscription) = subscription else { return };
        while let Some((position, offer)) = subscription.next().await {
            yield Ok(Event::default().data(json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": { "uri": uri, "_meta": { "xyz.inbound/offer": Wire::of(position, &offer) } }
            }).to_string()));
        }
    })
    .keep_alive(KeepAlive::default())
}
