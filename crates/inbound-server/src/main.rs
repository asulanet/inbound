mod feed;
mod mcp;
mod wire;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::{Router, http::StatusCode};
use commonware_cryptography::Signer;
use commonware_runtime::{Runner, Supervisor, tokio as cw};
use feed::{DAY, next_offer, sellers};
use futures::Stream;
use inbound_operator::ChannelLog;
use inbound_types::{ChannelId, ChannelSpec, FormatId, PrivateKey, Registry, RegistryBody};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use wire::{WireChannel, WireOffer};

/// Where the fake month starts. Offers carry real-looking windows relative to this.
const EPOCH: u64 = 1_790_812_800;

pub struct Server {
    pub registry: Registry,
    pub channel: ChannelId,
    pub log: Mutex<ChannelLog<cw::Context>>,
    pub base: String,
}

pub type Shared = Arc<Server>;

pub fn channel_slug(server: &Shared) -> String {
    server.channel.to_string()
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(7402);
    let seconds_per_offer: u64 = std::env::args()
        .nth(2)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(3);

    let runner = cw::Runner::default();
    runner.start(|context| async move {
        if let Err(error) = run(context, port, seconds_per_offer).await {
            eprintln!("server failed: {error}");
        }
    });
}

async fn run(
    context: cw::Context,
    port: u16,
    seconds_per_offer: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let maintainer = PrivateKey::from_seed(1);
    let operator = PrivateKey::from_seed(2);
    let channel = ChannelId::new("cashback/us/sportswear")?;
    let format = FormatId::new("cashback/v0")?;
    let base = format!("http://127.0.0.1:{port}");

    let spec = ChannelSpec {
        channel: channel.clone(),
        operator: operator.public_key(),
        format: format.clone(),
        min_bond: 50_000,
    };
    let registry = Registry::sign(
        &maintainer,
        RegistryBody {
            maintainer: maintainer.public_key(),
            channels: vec![spec.clone()],
            published_at: EPOCH,
        },
    );

    let log = ChannelLog::init(context.child("channel"), spec).await?;
    let server: Shared = Arc::new(Server {
        registry,
        channel: channel.clone(),
        log: Mutex::new(log),
        base: base.clone(),
    });

    // Sellers post on their own schedule. Nobody asked them to; they just keep arriving.
    let posting = server.clone();
    let seller_list = sellers();
    tokio::spawn(async move {
        let mut rng = feed::rng(2_026);
        let mut nonce = 0u64;
        let mut simulated_day = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(seconds_per_offer)).await;
            nonce += 1;
            simulated_day += 1;
            let now = EPOCH + simulated_day * DAY;
            let (index, offer) = next_offer(&mut rng, &seller_list, &channel, &format, now, nonce);
            let mut log = posting.log.lock().await;
            match log.post(&offer).await {
                Ok(position) => println!(
                    "  posted #{position} from {} to {} subscriber(s)",
                    seller_list[index].name,
                    log.subscribers()
                ),
                Err(error) => eprintln!("  rejected: {error}"),
            }
        }
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/registry", get(registry_handler))
        .route("/channels/{*channel}", get(channel_handler))
        .route("/mcp", get(mcp_manifest).post(mcp::endpoint))
        .with_state(server);

    let address: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("\ninbound channel server\n");
    println!("  registry   {base}/registry");
    println!("  channel    {base}/channels/cashback/us/sportswear");
    println!("  catch up   curl -s '{base}/channels/cashback/us/sportswear/offers?since=0' | jq");
    println!("  get pushed curl -N {base}/channels/cashback/us/sportswear/stream");
    println!("  mcp        {base}/mcp   (streamable http; tools + resource subscription)");
    println!("\n  a new offer lands every {seconds_per_offer}s. anything connected is told.\n");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn mcp_manifest(State(server): State<Shared>) -> impl IntoResponse {
    Json(mcp::manifest(&server.base))
}

async fn index(State(server): State<Shared>) -> impl IntoResponse {
    Json(wire::registry(&server.registry, &server.base))
}

async fn registry_handler(State(server): State<Shared>) -> impl IntoResponse {
    Json(wire::registry(&server.registry, &server.base))
}

#[derive(Deserialize)]
struct Since {
    since: Option<u64>,
}

/// One route serves the channel's three shapes: its description, its history from a cursor, and
/// its live stream. Trailing path segment decides which.
async fn channel_handler(
    Path(path): Path<String>,
    Query(since): Query<Since>,
    State(server): State<Shared>,
) -> axum::response::Response {
    let (name, action) = match path.rsplit_once('/') {
        Some((name, tail)) if tail == "stream" || tail == "offers" => (name, tail),
        _ => (path.as_str(), "info"),
    };
    if name != server.channel.as_str() {
        return (StatusCode::NOT_FOUND, format!("no channel {name}")).into_response();
    }

    match action {
        "stream" => stream(server).await.into_response(),
        "offers" => {
            let log = server.log.lock().await;
            match log.read_from(since.since.unwrap_or(0)).await {
                Ok(offers) => Json(
                    offers
                        .iter()
                        .map(|(position, offer)| WireOffer::new(*position, offer))
                        .collect::<Vec<_>>(),
                )
                .into_response(),
                Err(error) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                }
            }
        }
        _ => {
            let log = server.log.lock().await;
            let spec = log.spec();
            Json(WireChannel {
                channel: spec.channel.to_string(),
                operator: format!("{}", spec.operator),
                format: spec.format.to_string(),
                min_bond: spec.min_bond,
                tip: log.tip(),
                subscribers: log.subscribers(),
            })
            .into_response()
        }
    }
}

/// The push path. Holding this open is all an agent has to do; offers arrive as they are posted.
async fn stream(
    server: Shared,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut subscription = {
        let mut log = server.log.lock().await;
        log.subscribe()
    };
    let stream = async_stream::stream! {
        while let Some((position, offer)) = subscription.next().await {
            let payload = WireOffer::new(position, &offer);
            match serde_json::to_string(&payload) {
                Ok(json) => yield Ok(Event::default().event("offer").data(json)),
                Err(_) => continue,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
