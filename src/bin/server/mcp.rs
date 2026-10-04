//! The MCP adapter. Holds no state of its own.
//!
//! A channel is not an event type. The operator carries exactly one event type, `inbound.offer`,
//! and the channel an offer belongs to is a field on the payload — so an agent holds one
//! subscription with one cursor and sorts the channels out itself. Per-channel event types would
//! cost an agent a subscription each and would tell the operator which channels it keeps, which is
//! the one thing the feed is shaped to avoid.
//!
//! The event methods follow the draft Triggers and Events extension rather than the resource
//! subscriptions in revision 2026-07-28, whose stream dies with the transport and keeps no state
//! across reconnects:
//! <https://github.com/modelcontextprotocol/experimental-ext-triggers-events>
//!
//! Two places this departs from that draft, both deliberate. It replays from before the
//! subscription began, because a standing offer is still worth having when an agent arrives late.
//! And it ships the whole signed offer rather than a minimal payload to be fetched with a tool,
//! because fetching one would tell the operator which offers an agent cares about.

use crate::{PAGE, PAGE_LIMIT, Shared, Wire, iso8601, offers_since};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use futures::Stream;
use serde_json::{Value, json};
use std::time::Duration;

const EVENT: &str = "inbound.offer";

/// The draft's ceiling is 30s, and a client treats twice this with no frame as a dead stream.
const HEARTBEAT: Duration = Duration::from_secs(20);

pub async fn handle(State(operator): State<Shared>, body: String) -> Response {
    let Ok(request) = serde_json::from_str::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(id) = request.get("id").cloned() else {
        return StatusCode::ACCEPTED.into_response();
    };
    let cursor = request
        .pointer("/params/cursor")
        .and_then(Value::as_str)
        .and_then(|cursor| cursor.parse::<u64>().ok())
        .unwrap_or(0);

    let result = match request.get("method").and_then(Value::as_str).unwrap_or("") {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {
                "tools": {},
                "resources": {},
                "events": { "listChanged": false }
            },
            "serverInfo": { "name": "inbound", "version": "0.0.1" }
        }),
        "tools/list" => json!({ "tools": [
            { "name": "list_channels",
              "description": "Every channel this operator carries, and how to reach the feed.",
              "inputSchema": { "type": "object", "properties": {} } },
            { "name": "list_offers",
              "description": "Offers from a cursor onward, across every channel. Catch-up; live ones arrive on the subscription.",
              "inputSchema": { "type": "object", "properties": {
                  "since": { "type": "integer" } } } }
        ]}),
        "tools/call" => {
            let tool = request
                .pointer("/params/name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let text = if tool == "list_channels" {
                serde_json::to_string_pretty(&catalogue(&operator).await).unwrap_or_default()
            } else {
                let since = request
                    .pointer("/params/arguments/since")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                serde_json::to_string_pretty(&offers_since(&operator, since, PAGE).await)
                    .unwrap_or_default()
            };
            json!({ "content": [{ "type": "text", "text": text }], "isError": false })
        }
        // The catalogue is a resource and not an event type, so adding a channel never changes the
        // contract an agent subscribed under.
        "resources/list" => json!({ "resources": [{
            "uri": "inbound://channels",
            "name": "channels",
            "description": "Which channels this operator carries, and how much has been posted to each.",
            "mimeType": "application/json"
        }]}),
        "resources/read" => {
            let text =
                serde_json::to_string_pretty(&catalogue(&operator).await).unwrap_or_default();
            json!({ "contents": [{ "uri": "inbound://channels", "mimeType": "application/json", "text": text }] })
        }
        "events/list" => json!({ "events": [{
            "name": EVENT,
            "description": "A seller posted a signed offer to one of this operator's channels.",
            "delivery": ["push", "poll"],
            // No subscription arguments on purpose: a filter here would be evaluated by the
            // operator, which is how it would learn what an agent is looking for.
            "inputSchema": { "type": "object", "properties": {} },
            "payloadSchema": {
                "type": "object",
                "required": ["channel", "channel_seq", "seller", "reward_bps", "bytes"],
                "properties": {
                    "feed": { "type": "integer", "description": "Position in the feed; the cursor to resume from." },
                    "channel": { "type": "string" },
                    "channel_seq": { "type": "integer", "description": "Position within this offer's own channel, so a gap in one channel is visible without subscribing to it." },
                    "arrived": { "type": "integer", "description": "When the operator saw it. The operator's own word; judge liveness from expires inside the signature." },
                    "seller": { "type": "string" },
                    "reward_bps": { "type": "integer" },
                    "best_bps": { "type": "integer" },
                    "min_spend": { "type": "integer" },
                    "expires": { "type": "integer" },
                    "bytes": { "type": "string", "description": "The offer as the seller signed it. Verify before believing the rest." }
                }
            }
        }]}),
        "events/poll" => {
            let limit = request
                .pointer("/params/maxEvents")
                .and_then(Value::as_u64)
                .map(|limit| (limit as usize).min(PAGE_LIMIT))
                .unwrap_or(PAGE);
            let offers = offers_since(&operator, cursor, limit).await;
            let next = offers.last().map_or(cursor, |offer| offer.feed + 1);
            json!({
                "events": offers.iter().map(occurrence).collect::<Vec<_>>(),
                "cursor": next.to_string(),
                "truncated": false,
                "hasMore": offers.len() == limit,
                "nextPollMs": 15_000
            })
        }
        "events/stream" => {
            return stream(operator, id, cursor).await.into_response();
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

async fn catalogue(operator: &Shared) -> Value {
    let feed = operator.feed.lock().await;
    json!({
        "operator": "inbound demo",
        "event": EVENT,
        "offers": feed.tip(),
        "channels": feed.channels().iter().map(|(name, posted)| json!({
            "channel": name,
            "offers": posted,
        })).collect::<Vec<_>>(),
    })
}

fn occurrence(offer: &Wire) -> Value {
    json!({
        "eventId": format!("{EVENT}#{}", offer.feed),
        "name": EVENT,
        "timestamp": iso8601(offer.arrived),
        "data": offer,
        "cursor": (offer.feed + 1).to_string()
    })
}

/// Push delivery. The backlog is read while the subscription is taken, so an agent resuming from a
/// cursor sees no gap and no repeat at the join. A cursor past the end of the feed is clamped to
/// it: the agent is ahead of this operator, and every position reported back has to be one the
/// operator can stand behind.
async fn stream(
    operator: Shared,
    id: Value,
    cursor: u64,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (backlog, subscription, start) = {
        let mut feed = operator.feed.lock().await;
        let start = cursor.min(feed.tip());
        let backlog = feed.since(start).await.unwrap_or_default();
        (backlog, feed.subscribe(), start)
    };
    let meta = json!({ "io.modelcontextprotocol/subscriptionId": id });
    Sse::new(async_stream::stream! {
        let mut subscription = subscription;
        let mut at = start;
        yield Ok(frame("notifications/events/active", json!({
            "cursor": start.to_string(),
            "truncated": false,
            "_meta": meta
        })));
        for delivery in backlog {
            let wire = Wire::of(&delivery);
            at = wire.feed + 1;
            yield Ok(frame("notifications/events/event", with_meta(occurrence(&wire), &meta)));
        }
        let mut beat = tokio::time::interval(HEARTBEAT);
        beat.reset();
        loop {
            let delivery = tokio::select! {
                delivery = subscription.next() => delivery,
                _ = beat.tick() => {
                    // The draft rules out SSE comment keepalives here because they cannot carry a
                    // cursor, and a quiet feed would leave an agent holding a stale one.
                    yield Ok(frame("notifications/events/heartbeat", json!({
                        "cursor": at.to_string(),
                        "_meta": meta
                    })));
                    continue;
                }
            };
            let Some(delivery) = delivery else { return };
            let wire = Wire::of(&delivery);
            at = wire.feed + 1;
            yield Ok(frame("notifications/events/event", with_meta(occurrence(&wire), &meta)));
        }
    })
    .keep_alive(KeepAlive::default())
}

fn frame(method: &str, params: Value) -> Event {
    Event::default()
        .data(json!({ "jsonrpc": "2.0", "method": method, "params": params }).to_string())
}

fn with_meta(mut occurrence: Value, meta: &Value) -> Value {
    if let Some(fields) = occurrence.as_object_mut() {
        fields.insert("_meta".into(), meta.clone());
    }
    occurrence
}
