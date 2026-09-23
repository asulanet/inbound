//! A channel anything can connect to.
//!
//! Serves the offer log over plain HTTP, pushes new offers over SSE, and answers MCP so a
//! connector-capable agent attaches by URL. Fictional sellers post into it on a timer.

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::{Router, http::StatusCode};
use commonware_codec::Encode;
use commonware_runtime::{Runner, Supervisor, tokio as cw};
use futures::Stream;
use inbound::{Channel, Offer, name_of, sellers, write_offer};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const CHANNEL: &str = "cashback/us/sportswear";
const EPOCH: u64 = 1_790_812_800;

type Shared = Arc<Mutex<Channel<cw::Context>>>;

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
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(7402);
    let every: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(3);
    cw::Runner::default().start(|context| async move {
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
    let channel = Channel::open(context.child("channel"), CHANNEL).await?;
    let shared: Shared = Arc::new(Mutex::new(channel));

    let posting = shared.clone();
    tokio::spawn(async move {
        let names = sellers();
        let mut rng = ChaCha20Rng::seed_from_u64(2_026);
        let mut nonce = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(every)).await;
            nonce += 1;
            let (index, offer) = write_offer(&mut rng, CHANNEL, EPOCH + nonce * 86_400, nonce);
            let mut channel = posting.lock().await;
            match channel.post(&offer).await {
                Ok(position) => println!(
                    "  #{position} {} → {} listening",
                    names[index].name,
                    channel.subscribers()
                ),
                Err(error) => eprintln!("  refused: {error}"),
            }
        }
    });

    let base = format!("http://127.0.0.1:{port}");
    let app = Router::new()
        .route("/", get(index))
        .route("/channels/{*rest}", get(channel_route))
        .route("/mcp", get(index).post(mcp))
        .with_state(shared);

    println!("\ninbound · {CHANNEL}\n");
    println!("  catch up    curl -s '{base}/channels/{CHANNEL}/offers?since=0'");
    println!("  be told     curl -N {base}/channels/{CHANNEL}/stream");
    println!("  or by mcp   {base}/mcp");
    println!("\n  an offer lands every {every}s. whoever is listening is told.\n");

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(State(shared): State<Shared>) -> impl IntoResponse {
    let channel = shared.lock().await;
    Json(json!({
        "channel": channel.name(),
        "offers": channel.tip(),
        "listening": channel.subscribers(),
        "stream": format!("/channels/{}/stream", channel.name()),
        "offers_from": format!("/channels/{}/offers?since=0", channel.name()),
        "mcp": "/mcp"
    }))
}

#[derive(Deserialize)]
struct Since {
    since: Option<u64>,
}

async fn channel_route(
    Path(rest): Path<String>,
    Query(since): Query<Since>,
    State(shared): State<Shared>,
) -> Response {
    let (name, action) = match rest.rsplit_once('/') {
        Some((name, tail @ ("stream" | "offers"))) => (name, tail),
        _ => (rest.as_str(), ""),
    };
    if name != CHANNEL {
        return (StatusCode::NOT_FOUND, "no such channel").into_response();
    }
    match action {
        "stream" => stream(shared).await.into_response(),
        "offers" => Json(recent(&shared, since.since.unwrap_or(0)).await).into_response(),
        _ => index(State(shared)).await.into_response(),
    }
}

/// The push path. An agent holds this open and is told; it never asks.
async fn stream(
    shared: Shared,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut subscription = shared.lock().await.subscribe();
    Sse::new(async_stream::stream! {
        while let Some((position, offer)) = subscription.next().await {
            if let Ok(json) = serde_json::to_string(&Wire::of(position, &offer)) {
                yield Ok(Event::default().event("offer").data(json));
            }
        }
    })
    .keep_alive(KeepAlive::default())
}

async fn recent(shared: &Shared, since: u64) -> Vec<Wire> {
    let channel = shared.lock().await;
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
async fn mcp(State(shared): State<Shared>, body: String) -> Response {
    let Ok(request) = serde_json::from_str::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(id) = request.get("id").cloned() else {
        return StatusCode::ACCEPTED.into_response();
    };
    let uri = format!("inbound://{CHANNEL}");

    let result = match request.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {}, "resources": { "subscribe": true } },
            "serverInfo": { "name": "inbound", "version": "0.0.1" }
        }),
        "tools/list" => json!({ "tools": [
            { "name": "list_offers",
              "description": "Offers in the channel from a cursor onward. Catch-up; live ones arrive on the subscription.",
              "inputSchema": { "type": "object", "properties": { "since": { "type": "integer" } } } }
        ]}),
        "resources/list" => json!({ "resources": [
            { "uri": uri, "name": CHANNEL, "mimeType": "application/json" }
        ]}),
        "tools/call" | "resources/read" => {
            let since = request
                .pointer("/params/arguments/since")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let text =
                serde_json::to_string_pretty(&recent(&shared, since).await).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": text }], "isError": false })
        }
        "subscriptions/listen" => return mcp_stream(shared, id, uri).await.into_response(),
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

async fn mcp_stream(
    shared: Shared,
    id: Value,
    uri: String,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut subscription = shared.lock().await.subscribe();
    Sse::new(async_stream::stream! {
        yield Ok(Event::default().data(json!({
            "jsonrpc": "2.0",
            "method": "notifications/subscriptions/acknowledged",
            "params": { "_meta": { "io.modelcontextprotocol/subscriptionId": id } }
        }).to_string()));
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
