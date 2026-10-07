//! One feed anything can connect to.
//!
//! `/registry` says which channels the operator carries and how to reach the feed. The feed serves
//! its log over plain HTTP and pushes new offers over SSE. `/mcp` offers the same feed to a
//! connector-capable agent. Fictional sellers post on a timer.
//!
//! There is one feed rather than one per channel on purpose. An agent subscribes once and keeps
//! the channels its person cares about, so the operator never learns which those are.

mod mcp;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use commonware_codec::Encode;
use commonware_runtime::{Clock, Runner, Supervisor, tokio as cw};
use commonware_utils::SystemTimeExt;
use futures::Stream;
use inbound::{Delivery, Feed, name_of, next_seller, sellers, write_offer};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct Operator {
    pub base: String,
    pub feed: Mutex<Feed<cw::Context>>,
}

pub type Shared = Arc<Operator>;

/// The page, built into the binary so the demo is one command and one origin.
const SITE: &str = include_str!("site.html");

/// What an agent receives. `bytes` is the canonical encoding the seller signed, so an agent can
/// check the signature itself rather than trusting these readable fields. `feed` is the cursor to
/// resume from; `channel_seq` counts within this offer's own channel, so an agent keeping one
/// channel out of many can tell it missed something there without subscribing to it.
#[derive(Serialize)]
pub struct Wire {
    pub feed: u64,
    pub channel: String,
    pub channel_seq: u64,
    pub arrived: u64,
    pub seller: String,
    pub reward_bps: u16,
    pub reward_cap: u64,
    pub best_bps: u16,
    pub min_spend: u64,
    pub expires: u64,
    pub bytes: String,
}

impl Wire {
    pub fn of(delivery: &Delivery) -> Self {
        let offer = &delivery.offer;
        Self {
            feed: delivery.feed,
            channel: offer.body.channel.clone(),
            channel_seq: delivery.channel,
            arrived: delivery.arrived,
            seller: name_of(&offer.body.seller),
            reward_bps: offer.body.reward.basis_points,
            reward_cap: offer.body.reward.cap,
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

/// MCP wants an ISO 8601 instant where the feed keeps Unix seconds.
pub fn iso8601(seconds: u64) -> String {
    i64::try_from(seconds)
        .ok()
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|timestamp| timestamp.to_string())
        .unwrap_or_default()
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
    // Somewhere stable to keep the log, so a restart does not lose the feed. The runtime otherwise
    // picks a fresh temp directory each boot.
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
    let feed = Feed::open(context.child("feed")).await?;
    let channels = feed.channels();
    // Sellers take their place in the script from the log, so a restart continues rather than
    // replays what agents have already been told.
    let mut nonce = feed.tip();
    let operator: Shared = Arc::new(Operator {
        base: host.clone(),
        feed: Mutex::new(feed),
    });

    let posting = operator.clone();
    let ticker = context.child("sellers");
    tokio::spawn(async move {
        let shops = sellers();
        let mut rng = ChaCha20Rng::seed_from_u64(2_026);
        loop {
            tokio::time::sleep(Duration::from_secs(every)).await;
            nonce += 1;
            let seller = &shops[next_seller(&mut rng, &shops)];
            let arrived = ticker.current().epoch().as_secs();
            let offer = write_offer(&mut rng, seller, arrived, nonce);
            let mut feed = posting.feed.lock().await;
            match feed.post(&offer, arrived).await {
                Ok(delivery) => println!(
                    "  #{} {} {} → {} listening",
                    delivery.feed,
                    seller.channel,
                    seller.name,
                    feed.subscribers()
                ),
                Err(error) => eprintln!("  refused: {error}"),
            }
        }
    });

    let app = Router::new()
        .route("/", get(site))
        .route("/registry", get(registry))
        .route("/feed/stream", get(stream))
        .route("/feed/offers", get(offers))
        .route("/mcp", get(registry).post(mcp::handle))
        .with_state(operator);

    println!("\ninbound · one feed · {} channels\n", channels.len());
    for (name, _) in &channels {
        println!("  carries       {name}");
    }
    println!("\n  what exists   curl -s {host}/registry");
    println!("  be told       curl -N {host}/feed/stream");
    println!("  catch up      curl -s '{host}/feed/offers?since=0'");
    println!("  or by mcp     {host}/mcp");
    println!("  or in a browser {host}/");
    println!("\n  an offer lands every {every}s. whoever is listening is told.\n");

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// The page a person lands on. It is a reader like any other: it subscribes to the whole feed and
/// decides on its own side, in the browser, against numbers nothing sends back. Served from the
/// same origin as the feed so it needs no cross-origin grant.
async fn site() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], SITE)
}

/// How an agent finds anything. One URL in, everything it needs to subscribe out. It lists the
/// channels so an agent knows what exists, and one stream carrying all of them so that reading it
/// reveals nothing about which ones it wants.
async fn registry(State(operator): State<Shared>) -> impl IntoResponse {
    let feed = operator.feed.lock().await;
    Json(json!({
        "operator": "inbound demo",
        "offers": feed.tip(),
        "listening": feed.subscribers(),
        "stream": format!("{}/feed/stream", operator.base),
        "since": format!("{}/feed/offers?since=0", operator.base),
        "mcp": format!("{}/mcp", operator.base),
        "channels": feed.channels().iter().map(|(name, posted)| json!({
            "channel": name,
            "offers": posted,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
pub struct Page {
    pub since: Option<u64>,
    pub limit: Option<usize>,
}

const PAGE: usize = 200;
const PAGE_LIMIT: usize = 500;

pub async fn offers_since(operator: &Shared, since: u64, limit: usize) -> Vec<Wire> {
    let feed = operator.feed.lock().await;
    feed.since(since)
        .await
        .map(|deliveries| deliveries.iter().take(limit).map(Wire::of).collect())
        .unwrap_or_default()
}

async fn offers(Query(page): Query<Page>, State(operator): State<Shared>) -> impl IntoResponse {
    let limit = page.limit.unwrap_or(PAGE).min(PAGE_LIMIT);
    Json(offers_since(&operator, page.since.unwrap_or(0), limit).await)
}

/// The push path. An agent holds this open and is told; it never asks. With `?since=N` the backlog
/// goes out first: the subscription is taken while the backlog is read, so nothing lands in between
/// and the positions an agent sees stay consecutive.
async fn stream(
    Query(page): Query<Page>,
    State(operator): State<Shared>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (backlog, subscription) = {
        let mut feed = operator.feed.lock().await;
        let backlog = match page.since {
            Some(since) => feed.since(since).await.unwrap_or_default(),
            None => Vec::new(),
        };
        (backlog, feed.subscribe())
    };
    Sse::new(async_stream::stream! {
        let mut subscription = subscription;
        for delivery in backlog {
            if let Ok(json) = serde_json::to_string(&Wire::of(&delivery)) {
                yield Ok(Event::default().event("offer").id(delivery.feed.to_string()).data(json));
            }
        }
        while let Some(delivery) = subscription.next().await {
            if let Ok(json) = serde_json::to_string(&Wire::of(&delivery)) {
                yield Ok(Event::default().event("offer").id(delivery.feed.to_string()).data(json));
            }
        }
    })
    .keep_alive(KeepAlive::default())
}
