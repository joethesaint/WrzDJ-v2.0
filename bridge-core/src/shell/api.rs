//! The Electron app's server calls: `auth-service.ts`, `bridge-api-key-service.ts`,
//! `events-service.ts` and `event-health-service.ts`.
//!
//! Same endpoints, statuses and messages. Stricter than the TypeScript in three places, each
//! where the TypeScript would hand back `undefined` or a raw runtime error: a success reply
//! without `bridge_api_key`, an event list that isn't a list of events, and a sign-in the
//! server never answers ("Login unavailable", where Node says "fetch failed").

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Per-request limit; `check_event_health` uses it as the TypeScript does (10 s).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub message: String,
    /// The transport or body error behind an "unavailable" error.
    pub cause: Option<String>,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        ApiError {
            message: message.into(),
            cause: None,
        }
    }
    fn caused(message: impl Into<String>, cause: String) -> Self {
        ApiError {
            message: message.into(),
            cause: Some(cause),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginResult {
    pub access_token: String,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub is_authenticated: bool,
    pub username: Option<String>,
    pub api_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInfo {
    pub id: i64,
    /// Internal collection code: the identifier the Bridge API resolves by.
    pub code: String,
    /// Live join code: the one guests use.
    pub join_code: String,
    pub name: String,
    pub is_active: bool,
    pub expires_at: String,
}

#[derive(Deserialize)]
struct EventResponse {
    id: i64,
    code: String,
    join_code: String,
    name: String,
    is_active: bool,
    expires_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHealth {
    /// 200: the event exists and is live.
    Active,
    /// 404: the event was deleted.
    NotFound,
    /// 410: the event expired or was archived.
    Expired,
    /// Anything else (network, server error): don't act on it.
    Error,
}

struct Reply {
    status: u16,
    body: Result<String, String>,
}

impl Reply {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    fn json(&self) -> Option<Value> {
        self.body
            .as_ref()
            .ok()
            .and_then(|b| serde_json::from_str(b).ok())
    }
    /// `detail` when it is a non-blank string.
    fn detail(&self) -> Option<String> {
        match self.json()?.get("detail")? {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        }
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

fn read(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Reply, String> {
    match response {
        Ok(mut r) => {
            let status = r.status().as_u16();
            let body = r.body_mut().read_to_string().map_err(|e| e.to_string());
            Ok(Reply { status, body })
        }
        Err(e) => Err(e.to_string()),
    }
}

fn get(url: &str, token: Option<&str>, timeout: Duration) -> Result<Reply, String> {
    let mut request = agent(timeout).get(url);
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    read(request.call())
}

/// Sign in with the dashboard's OAuth2 password flow (`POST /api/auth/login`, form-encoded).
pub fn login(api_url: &str, username: &str, password: &str) -> Result<LoginResult, ApiError> {
    let form = format!(
        "username={}&password={}",
        form_encode(username),
        form_encode(password)
    );
    let response = agent(REQUEST_TIMEOUT)
        .post(&format!("{api_url}/api/auth/login"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form);
    let reply = read(response).map_err(|e| ApiError::caused("Login unavailable", e))?;
    if !reply.ok() {
        return Err(ApiError::new(
            reply.detail().unwrap_or_else(|| "Login failed".into()),
        ));
    }
    match reply.json().as_ref().and_then(|v| v.get("access_token")) {
        Some(Value::String(t)) if !t.trim().is_empty() => Ok(LoginResult {
            access_token: t.clone(),
            username: username.to_string(),
        }),
        _ => Err(ApiError::new(
            "Login response did not include a valid access token",
        )),
    }
}

/// Check a stored token (`GET /api/auth/me`). `Ok(None)` only for 401/403, so a transient
/// outage never signs the DJ out; transport, server and malformed replies are errors.
pub fn verify_token(api_url: &str, token: &str) -> Result<Option<String>, ApiError> {
    let reply = get(
        &format!("{api_url}/api/auth/me"),
        Some(token),
        REQUEST_TIMEOUT,
    )
    .map_err(|e| ApiError::caused("Token verification unavailable", e))?;
    if reply.status == 401 || reply.status == 403 {
        return Ok(None);
    }
    if !reply.ok() {
        return Err(ApiError::new(format!(
            "Token verification failed ({})",
            reply.status
        )));
    }
    if let Err(e) = &reply.body {
        return Err(ApiError::caused(
            "Token verification unavailable",
            e.clone(),
        ));
    }
    match reply.json() {
        Some(Value::Object(map)) => match map.get("username") {
            Some(Value::String(u)) if !u.trim().is_empty() => Ok(Some(u.clone())),
            _ => Err(ApiError::new(
                "Token verification returned an invalid username",
            )),
        },
        _ => Err(ApiError::new(
            "Token verification returned an invalid username",
        )),
    }
}

pub fn build_auth_state(api_url: &str, username: Option<&str>) -> AuthState {
    AuthState {
        is_authenticated: username.is_some(),
        username: username.map(str::to_string),
        api_url: api_url.to_string(),
    }
}

/// The shared Bridge API key, for a signed-in DJ (`GET /api/bridge/apikey`).
pub fn fetch_bridge_api_key(api_url: &str, token: &str) -> Result<String, ApiError> {
    const FAILED: &str = "Failed to fetch bridge API key";
    let reply = get(
        &format!("{api_url}/api/bridge/apikey"),
        Some(token),
        REQUEST_TIMEOUT,
    )
    .map_err(|e| ApiError::caused(FAILED, e))?;
    match reply.status {
        s if (200..300).contains(&s) => {}
        401 => return Err(ApiError::new("Session expired. Please log in again.")),
        404 => {
            return Err(ApiError::new(
                "Bridge API key endpoint not found. Make sure the server is updated to the latest version.",
            ));
        }
        503 => return Err(ApiError::new("Bridge API key not configured on server.")),
        _ => {
            return Err(ApiError::new(
                reply.detail().unwrap_or_else(|| FAILED.into()),
            ));
        }
    }
    match reply.json().as_ref().and_then(|v| v.get("bridge_api_key")) {
        Some(Value::String(k)) if !k.is_empty() => Ok(k.clone()),
        _ => Err(ApiError::new(FAILED)),
    }
}

/// The signed-in DJ's events, active ones only (`GET /api/events`).
pub fn fetch_events(api_url: &str, token: &str) -> Result<Vec<EventInfo>, ApiError> {
    const FAILED: &str = "Failed to fetch events";
    let reply = get(
        &format!("{api_url}/api/events"),
        Some(token),
        REQUEST_TIMEOUT,
    )
    .map_err(|e| ApiError::caused(FAILED, e))?;
    if !reply.ok() {
        if reply.status == 401 {
            return Err(ApiError::new("Session expired. Please log in again."));
        }
        return Err(ApiError::new(
            reply.detail().unwrap_or_else(|| FAILED.into()),
        ));
    }
    let body = reply.body.map_err(|e| ApiError::caused(FAILED, e))?;
    let events: Vec<EventResponse> =
        serde_json::from_str(&body).map_err(|e| ApiError::caused(FAILED, e.to_string()))?;
    Ok(events
        .into_iter()
        .filter(|e| e.is_active)
        .map(|e| EventInfo {
            id: e.id,
            code: e.code,
            join_code: e.join_code,
            name: e.name,
            is_active: e.is_active,
            expires_at: e.expires_at,
        })
        .collect())
}

/// Does the event still exist (`GET /api/public/events/{code}`, no auth)? This endpoint takes the
/// collection code or the join code; the join-code-only one would misread a live event as gone.
pub fn check_event_health(api_url: &str, event_code: &str) -> EventHealth {
    check_event_health_within(api_url, event_code, REQUEST_TIMEOUT)
}

pub fn check_event_health_within(
    api_url: &str,
    event_code: &str,
    timeout: Duration,
) -> EventHealth {
    let url = format!(
        "{api_url}/api/public/events/{}",
        encode_uri_component(event_code)
    );
    match get(&url, None, timeout) {
        Ok(r) if r.ok() => EventHealth::Active,
        Ok(r) if r.status == 404 => EventHealth::NotFound,
        Ok(r) if r.status == 410 => EventHealth::Expired,
        _ => EventHealth::Error,
    }
}

/// JavaScript's `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    encode(
        s,
        |b| b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b),
        false,
    )
}

/// `URLSearchParams` (application/x-www-form-urlencoded): space becomes `+`.
fn form_encode(s: &str) -> String {
    encode(
        s,
        |b| b.is_ascii_alphanumeric() || b"*-._".contains(&b),
        true,
    )
}

fn encode(s: &str, keep: impl Fn(u8) -> bool, plus_for_space: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if keep(b) {
            out.push(b as char);
        } else if plus_for_space && b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::mock::{Reply as R, serve};

    // login (auth-service.test.ts)
    #[test]
    fn login_sends_form_post_and_returns_token_and_username() {
        let (url, seen) = serve(vec![R::Status(200, r#"{"access_token":"jwt-token-123"}"#)]);
        let got = login(&url, "admin", "p@ss word").unwrap();
        assert_eq!(
            got,
            LoginResult {
                access_token: "jwt-token-123".into(),
                username: "admin".into()
            }
        );
        let req = seen.recv().unwrap();
        assert_eq!(
            (req.method.as_str(), req.path.as_str()),
            ("POST", "/api/auth/login")
        );
        assert_eq!(
            req.header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(req.body, "username=admin&password=p%40ss+word");
    }

    #[test]
    fn login_failure_uses_detail_or_generic_message() {
        let (url, _s) = serve(vec![
            R::Status(401, r#"{"detail":"Invalid credentials"}"#),
            R::Status(500, "<html>"),
            R::Status(400, r#"{"detail":"   "}"#),
            R::Status(400, r#"{"detail":42}"#),
        ]);
        assert_eq!(
            login(&url, "a", "b").unwrap_err().message,
            "Invalid credentials"
        );
        for _ in 0..3 {
            assert_eq!(login(&url, "a", "b").unwrap_err().message, "Login failed");
        }
    }

    #[test]
    fn login_rejects_malformed_and_unparseable_success_bodies() {
        let bodies = [
            "{}",
            r#"{"access_token":""}"#,
            r#"{"access_token":"  "}"#,
            r#"{"access_token":5}"#,
            "null",
            "<html>",
        ];
        let (url, _s) = serve(bodies.iter().map(|b| R::Status(200, b)).collect());
        for _ in bodies {
            assert_eq!(
                login(&url, "a", "b").unwrap_err().message,
                "Login response did not include a valid access token"
            );
        }
    }

    #[test]
    fn login_preserves_a_valid_access_token_exactly() {
        let (url, _s) = serve(vec![R::Status(
            200,
            r#"{"access_token":"  jwt-token-123  "}"#,
        )]);
        assert_eq!(
            login(&url, "admin", "p").unwrap().access_token,
            "  jwt-token-123  "
        );
    }

    #[test]
    fn login_with_no_answer_is_unavailable() {
        let (url, _s) = serve(vec![R::Close]);
        let err = login(&url, "a", "b").unwrap_err();
        assert_eq!(err.message, "Login unavailable");
        assert!(err.cause.is_some());
    }

    // verifyToken
    #[test]
    fn verify_returns_username_and_sends_bearer() {
        let (url, seen) = serve(vec![R::Status(200, r#"{"username":"  dj  "}"#)]);
        assert_eq!(verify_token(&url, "tok").unwrap(), Some("  dj  ".into()));
        let req = seen.recv().unwrap();
        assert_eq!(req.path, "/api/auth/me");
        assert_eq!(req.header("authorization"), Some("Bearer tok"));
    }

    #[test]
    fn verify_returns_none_only_for_401_and_403() {
        let (url, _s) = serve(vec![R::Status(401, ""), R::Status(403, "")]);
        assert_eq!(verify_token(&url, "t").unwrap(), None);
        assert_eq!(verify_token(&url, "t").unwrap(), None);
    }

    #[test]
    fn verify_errors_on_network_server_and_malformed_replies() {
        let (url, _s) = serve(vec![
            R::Close,
            R::Status(500, ""),
            R::Status(200, "{}"),
            R::Status(200, "<html>"),
            R::Status(200, r#"[{"username":"dj"}]"#),
            R::Status(200, r#"{"username":" "}"#),
            R::Truncated(200),
        ]);
        let msg = |r: Result<Option<String>, ApiError>| r.unwrap_err().message;
        assert_eq!(
            msg(verify_token(&url, "t")),
            "Token verification unavailable"
        );
        assert_eq!(
            msg(verify_token(&url, "t")),
            "Token verification failed (500)"
        );
        for _ in 0..4 {
            assert_eq!(
                msg(verify_token(&url, "t")),
                "Token verification returned an invalid username"
            );
        }
        let err = verify_token(&url, "t").unwrap_err();
        assert_eq!(err.message, "Token verification unavailable");
        assert!(err.cause.is_some(), "a body-stream failure keeps its cause");
    }

    #[test]
    fn builds_auth_state() {
        let on = build_auth_state("https://api.wrzdj.com", Some("dj"));
        assert!(on.is_authenticated && on.username.as_deref() == Some("dj"));
        let off = build_auth_state("https://api.wrzdj.com", None);
        assert!(!off.is_authenticated && off.username.is_none());
    }

    // fetchBridgeApiKey
    #[test]
    fn api_key_success_and_status_messages() {
        let (url, seen) = serve(vec![
            R::Status(200, r#"{"bridge_api_key":"k-123"}"#),
            R::Status(401, ""),
            R::Status(404, ""),
            R::Status(503, ""),
            R::Status(500, r#"{"detail":"Boom"}"#),
            R::Status(500, "<html>"),
            R::Status(200, "{}"),
        ]);
        assert_eq!(fetch_bridge_api_key(&url, "tok").unwrap(), "k-123");
        assert_eq!(
            seen.recv().unwrap().header("authorization"),
            Some("Bearer tok")
        );
        let msg = || fetch_bridge_api_key(&url, "tok").unwrap_err().message;
        assert_eq!(msg(), "Session expired. Please log in again.");
        assert!(msg().starts_with("Bridge API key endpoint not found."));
        assert_eq!(msg(), "Bridge API key not configured on server.");
        assert_eq!(msg(), "Boom");
        assert_eq!(msg(), "Failed to fetch bridge API key");
        assert_eq!(
            msg(),
            "Failed to fetch bridge API key",
            "success without a key is an error"
        );
    }

    // fetchEvents
    #[test]
    fn events_are_transformed_and_filtered_to_active() {
        let body = r#"[{"id":1,"code":"C1","join_code":"J1","name":"Live","is_active":true,"expires_at":"2026-11-07T23:00:00Z"},
                       {"id":2,"code":"C2","join_code":"J2","name":"Old","is_active":false,"expires_at":"2026-10-01T00:00:00Z"}]"#;
        let (url, seen) = serve(vec![R::Status(200, body), R::Status(200, "[]")]);
        let events = fetch_events(&url, "tok").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            (events[0].code.as_str(), events[0].join_code.as_str()),
            ("C1", "J1")
        );
        assert_eq!(seen.recv().unwrap().path, "/api/events");
        assert!(fetch_events(&url, "tok").unwrap().is_empty());
    }

    #[test]
    fn events_errors() {
        let (url, _s) = serve(vec![
            R::Status(401, ""),
            R::Status(500, r#"{"detail":"Nope"}"#),
            R::Status(200, "{}"),
        ]);
        assert_eq!(
            fetch_events(&url, "t").unwrap_err().message,
            "Session expired. Please log in again."
        );
        assert_eq!(fetch_events(&url, "t").unwrap_err().message, "Nope");
        assert_eq!(
            fetch_events(&url, "t").unwrap_err().message,
            "Failed to fetch events"
        );
    }

    // checkEventHealth
    #[test]
    fn health_maps_statuses() {
        let (url, _s) = serve(vec![
            R::Status(200, "{}"),
            R::Status(404, ""),
            R::Status(410, ""),
            R::Status(500, ""),
            R::Close,
        ]);
        assert_eq!(check_event_health(&url, "E"), EventHealth::Active);
        assert_eq!(check_event_health(&url, "E"), EventHealth::NotFound);
        assert_eq!(check_event_health(&url, "E"), EventHealth::Expired);
        assert_eq!(check_event_health(&url, "E"), EventHealth::Error);
        assert_eq!(check_event_health(&url, "E"), EventHealth::Error);
    }

    #[test]
    fn health_encodes_the_event_code() {
        let (url, seen) = serve(vec![R::Status(200, "{}")]);
        check_event_health(&url, "A B/C?d=é");
        assert_eq!(
            seen.recv().unwrap().path,
            "/api/public/events/A%20B%2FC%3Fd%3D%C3%A9"
        );
    }

    #[test]
    fn health_times_out_as_error() {
        let (url, _s) = serve(vec![R::Hang(2_000)]);
        let started = std::time::Instant::now();
        let got = check_event_health_within(&url, "E", Duration::from_millis(200));
        assert_eq!(got, EventHealth::Error);
        assert!(started.elapsed() < Duration::from_millis(1_500));
    }

    #[test]
    fn encoders_match_javascript() {
        assert_eq!(encode_uri_component("-_.!~*'()aZ9"), "-_.!~*'()aZ9");
        assert_eq!(encode_uri_component("a b+c&é"), "a%20b%2Bc%26%C3%A9");
        assert_eq!(form_encode("a b+c*~"), "a+b%2Bc*%7E");
    }
}
