//! An MCP face on the channel, so any connector-capable agent can attach by URL.
//!
//! Tools cover the pull side (discover channels, catch up from a cursor). Push is the resource
//! subscription: a client that opens `subscriptions/listen` holds a stream and is told
//! `notifications/resources/updated` as each offer lands, which is the same fan-out the SSE
//! endpoint uses.

use crate::wire::WireOffer;
use crate::{Shared, channel_slug};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures::Stream;
use serde_json::{Value, json};

pub const PROTOCOL: &str = "2025-06-18";

/// The single MCP endpoint. Requests that produce a stream answer with SSE; everything else
/// answers with JSON.
pub async fn endpoint(State(server): State<Shared>, _headers: HeaderMap, body: String) -> Response {
    let Ok(request) = serde_json::from_str::<Value>(&body) else {
        return error(None, -32700, "parse error").into_response();
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    // Notifications carry no id and expect no body.
    if id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    match method {
        "initialize" => {
            let mut response = Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": PROTOCOL,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": true, "listChanged": false }
                    },
                    "serverInfo": { "name": "inbound-channel", "version": "0.0.1" }
                }
            }))
            .into_response();
            if let Ok(value) = "inbound-session".parse() {
                response.headers_mut().insert("mcp-session-id", value);
            }
            response
        }
        "ping" => Json(json!({"jsonrpc": "2.0", "id": id, "result": {}})).into_response(),
        "tools/list" => Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": tools() }
        }))
        .into_response(),
        "resources/list" => {
            let slug = channel_slug(&server);
            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "resources": [{
                    "uri": format!("inbound://{slug}"),
                    "name": slug,
                    "title": "Cashback offers, US sportswear",
                    "description": "Offers posted by sellers into this channel. Subscribe to be told as they land.",
                    "mimeType": "application/json"
                }]}
            }))
            .into_response()
        }
        "resources/read" => {
            let offers = read_offers(&server, 0).await;
            let slug = channel_slug(&server);
            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "contents": [{
                    "uri": format!("inbound://{slug}"),
                    "mimeType": "application/json",
                    "text": serde_json::to_string(&offers).unwrap_or_default()
                }]}
            }))
            .into_response()
        }
        "subscriptions/listen" => subscribe(server, id).await.into_response(),
        "tools/call" => call(server, id, request.get("params").cloned()).await,
        _ => error(id, -32601, "method not found").into_response(),
    }
    .into_response()
}

fn tools() -> Value {
    json!([
        {
            "name": "list_channels",
            "description": "List the channels this operator runs, with the offer format each expects.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "list_offers",
            "description": "Offers posted to a channel from a cursor onward. Use this to catch up after being offline; live offers arrive on the subscription.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "since": { "type": "integer", "description": "Position to read from. 0 for everything." }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "channel_status",
            "description": "How many offers the channel holds and how many agents are subscribed.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }
    ])
}

async fn call(server: Shared, id: Option<Value>, params: Option<Value>) -> Response {
    let name = params
        .as_ref()
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let arguments = params
        .as_ref()
        .and_then(|params| params.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let text = match name {
        "list_channels" => {
            serde_json::to_string_pretty(&crate::wire::registry(&server.registry, &server.base))
                .unwrap_or_default()
        }
        "list_offers" => {
            let since = arguments.get("since").and_then(Value::as_u64).unwrap_or(0);
            let offers = read_offers(&server, since).await;
            serde_json::to_string_pretty(&offers).unwrap_or_default()
        }
        "channel_status" => {
            let log = server.log.lock().await;
            json!({
                "channel": log.spec().channel.to_string(),
                "offers": log.tip(),
                "subscribers": log.subscribers()
            })
            .to_string()
        }
        _ => return error(id, -32602, "unknown tool").into_response(),
    };

    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }], "isError": false }
    }))
    .into_response()
}

/// The push path. The stream stays open and carries a notification per offer. Nothing survives a
/// disconnect, which is why `list_offers` exists alongside it.
async fn subscribe(
    server: Shared,
    id: Option<Value>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let slug = channel_slug(&server);
    let mut subscription = {
        let mut log = server.log.lock().await;
        log.subscribe()
    };
    let acknowledged = json!({
        "jsonrpc": "2.0",
        "method": "notifications/subscriptions/acknowledged",
        "params": { "_meta": { "io.modelcontextprotocol/subscriptionId": id } }
    });

    let stream = async_stream::stream! {
        yield Ok(Event::default().data(acknowledged.to_string()));
        while let Some((position, offer)) = subscription.next().await {
            let payload = WireOffer::new(position, &offer);
            let notification = json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": {
                    "uri": format!("inbound://{slug}"),
                    "_meta": {
                        "io.modelcontextprotocol/subscriptionId": id,
                        "xyz.inbound/offer": payload
                    }
                }
            });
            yield Ok(Event::default().data(notification.to_string()));
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn read_offers(server: &Shared, since: u64) -> Vec<WireOffer> {
    let log = server.log.lock().await;
    log.read_from(since)
        .await
        .map(|offers| {
            offers
                .iter()
                .map(|(position, offer)| WireOffer::new(*position, offer))
                .collect()
        })
        .unwrap_or_default()
}

fn error(id: Option<Value>, code: i32, message: &str) -> Json<Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }))
}

/// Advertised so a client knows what it is looking at before it subscribes.
pub fn manifest(base: &str) -> Value {
    json!({
        "name": "inbound",
        "description": "A channel businesses post offers into. Subscribe and they are pushed to you.",
        "mcp": format!("{base}/mcp"),
        "transport": "streamable-http",
        "protocolVersion": PROTOCOL,
        "headers": { "content-type": "application/json", "accept": "application/json, text/event-stream" }
    })
}
