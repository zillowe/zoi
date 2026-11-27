//! A minimal `PostHog` capture client.
//!
//! Zoi only ever sends one analytics event at a time from the detached
//! flusher (see [`crate::queue`]), so this is a single blocking request to
//! `PostHog`'s capture endpoint (`/i/v1/analytics/events`), using the same
//! request format as the official SDK: a `batch` of events, authenticated
//! with the project key as a bearer token, answered with a result per event
//! UUID.
//!
//! Privacy: every event carries `$geoip_disable: true`, so `PostHog` does not
//! derive a location from the connection, and nothing beyond the queued
//! properties is added except the library name and version.

use std::time::Duration;

use serde::Deserialize;

/// Path of `PostHog`'s capture endpoint, relative to the ingest host.
const CAPTURE_PATH: &str = "/i/v1/analytics/events";

/// Identifies this client to `PostHog` (`$lib`, `user-agent`).
const LIB_NAME: &str = "zoi.telemetry";

/// What happened to one delivered event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Stored by `PostHog` (`ok`, or `warning` for a stored event with a
    /// note).
    Stored,
    /// Refused for good (`drop`); sending it again would not help.
    Dropped,
    /// Not stored this time (`retry`, an unknown result, or an error status
    /// from the server); keep it and try again later.
    Retry
}

/// A blocking client for one `PostHog` project.
pub struct Client {
    /// Project API key (`phc_...`).
    api_key: String,
    /// Full capture URL.
    url: String,
    /// HTTP client with the request timeout applied.
    http: reqwest::blocking::Client
}

/// The per-event results of a capture request.
#[derive(Debug, Deserialize)]
struct CaptureResponse {
    /// Result per event UUID.
    #[serde(default)]
    results: std::collections::HashMap<String, EventResult>
}

/// The result `PostHog` reports for one event.
#[derive(Debug, Deserialize)]
struct EventResult {
    /// `ok`, `warning`, `drop` or `retry`.
    result: String
}

impl Client {
    /// Builds a client for `host` (such as `https://eu.i.posthog.com`).
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(
        api_key: &str,
        host: &str,
        timeout: Duration
    ) -> Result<Self, reqwest::Error> {
        let http = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .use_rustls_tls()
            .build()?;
        Ok(Self {
            api_key: api_key.to_string(),
            url: format!("{}{CAPTURE_PATH}", host.trim_end_matches('/')),
            http
        })
    }

    /// Sends one event and reports what `PostHog` did with it.
    ///
    /// # Errors
    ///
    /// Returns an error when the request could not be made or answered at
    /// all (no network, DNS, refused connection, timeout). The caller should
    /// stop sending then, since every other event would fail the same way.
    pub fn capture(
        &self,
        event: &str,
        distinct_id: &str,
        properties: &serde_json::Map<String, serde_json::Value>
    ) -> Result<Delivery, reqwest::Error> {
        let uuid = new_uuid();
        let now = jiff::Timestamp::now().to_string();
        let body = build_request(event, distinct_id, properties, &uuid, &now);
        let user_agent = format!("{LIB_NAME}/{}", env!("CARGO_PKG_VERSION"));
        let response = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("user-agent", &user_agent)
            .header("posthog-sdk-info", &user_agent)
            .header("posthog-attempt", "1")
            .header("posthog-request-id", new_uuid())
            .header("posthog-request-timestamp", &now)
            .body(body.to_string())
            .send()?;
        if !response.status().is_success() {
            return Ok(Delivery::Retry);
        }
        let text = response.text()?;
        Ok(delivery_from_response(&text, &uuid))
    }
}

/// Builds the JSON body of a capture request for one event.
#[must_use]
pub fn build_request(
    event: &str,
    distinct_id: &str,
    properties: &serde_json::Map<String, serde_json::Value>,
    uuid: &str,
    timestamp: &str
) -> serde_json::Value {
    let mut properties = properties.clone();
    properties.insert("$geoip_disable".into(), true.into());
    properties.entry("$lib").or_insert_with(|| LIB_NAME.into());
    properties
        .entry("$lib_version")
        .or_insert_with(|| env!("CARGO_PKG_VERSION").into());
    serde_json::json!({
        "created_at": timestamp,
        "batch": [{
            "event": event,
            "uuid": uuid,
            "distinct_id": distinct_id,
            "timestamp": timestamp,
            "options": {},
            "properties": properties,
        }],
    })
}

/// Reads what happened to the event `uuid` from a capture response body.
#[must_use]
pub fn delivery_from_response(body: &str, uuid: &str) -> Delivery {
    let Ok(response) = serde_json::from_str::<CaptureResponse>(body) else {
        return Delivery::Retry;
    };
    match response.results.get(uuid).map(|r| r.result.as_str()) {
        Some("ok" | "warning") => Delivery::Stored,
        Some("drop") => Delivery::Dropped,
        _ => Delivery::Retry
    }
}

/// A new time-ordered (v7) UUID, as `PostHog`'s own SDKs use.
fn new_uuid() -> String {
    let ts = uuid::Timestamp::from_unix(
        uuid::NoContext,
        jiff::Timestamp::now().as_millisecond().cast_unsigned(),
        0
    );
    uuid::Uuid::new_v7(ts).to_string()
}

#[cfg(test)]
mod tests {
    use super::{Delivery, build_request, delivery_from_response};

    #[test]
    fn request_carries_the_event_and_disables_geoip() {
        let mut props = serde_json::Map::new();
        props.insert("command".into(), "install".into());
        let body = build_request(
            "install",
            "client-1",
            &props,
            "0190-uuid",
            "2026-11-28T00:00:00Z"
        );
        let event = body
            .get("batch")
            .and_then(|b| b.get(0))
            .expect("one event in the batch");
        assert_eq!(event.get("event"), Some(&"install".into()));
        assert_eq!(event.get("uuid"), Some(&"0190-uuid".into()));
        assert_eq!(event.get("distinct_id"), Some(&"client-1".into()));
        let props = event.get("properties").expect("properties");
        assert_eq!(props.get("command"), Some(&"install".into()));
        assert_eq!(props.get("$geoip_disable"), Some(&true.into()));
        assert_eq!(props.get("$lib"), Some(&"zoi.telemetry".into()));
    }

    #[test]
    fn results_map_to_deliveries() {
        let body =
            |r: &str| format!(r#"{{"results":{{"u1":{{"result":"{r}"}}}}}}"#);
        assert_eq!(delivery_from_response(&body("ok"), "u1"), Delivery::Stored);
        assert_eq!(
            delivery_from_response(&body("warning"), "u1"),
            Delivery::Stored
        );
        assert_eq!(
            delivery_from_response(&body("drop"), "u1"),
            Delivery::Dropped
        );
        assert_eq!(
            delivery_from_response(&body("retry"), "u1"),
            Delivery::Retry
        );
        assert_eq!(
            delivery_from_response(&body("ok"), "other"),
            Delivery::Retry,
            "an event missing from the results was not stored"
        );
        assert_eq!(delivery_from_response("not json", "u1"), Delivery::Retry);
    }
}
