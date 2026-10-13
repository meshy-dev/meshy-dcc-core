//! Browser guard for every gateway route.
//!
//! Any web page the user opens can send requests to `127.0.0.1:19765`, and a
//! page on a domain that re-resolves to 127.0.0.1 (DNS rebinding) even looks
//! same-origin. Browser requests (they carry `Origin` or `Sec-Fetch-Site`) are
//! therefore accepted only when:
//!
//! - the `Host` they name cannot be rebound: an IP literal, `localhost`,
//!   `*.localhost`, or a name in `MESHY_DCC_GATEWAY_ALLOWED_HOSTS`; and
//! - any `Origin` is the gateway's own address, a loopback page talking to a
//!   loopback gateway, or listed in `MESHY_DCC_GATEWAY_ALLOWED_ORIGINS`.
//!
//! Non-browser clients (the CLI, adapters, curl) send neither header and are
//! not affected, so remote CLI profiles and LAN registration keep working. A
//! remote browser that opens `http://<gateway IP>:19765/admin/` is same-origin
//! and passes; one that uses a host name needs that name in
//! `MESHY_DCC_GATEWAY_ALLOWED_HOSTS`. Both lists are comma-separated; `*`
//! allows everything.

use std::net::IpAddr;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub const ALLOWED_HOSTS_ENV: &str = "MESHY_DCC_GATEWAY_ALLOWED_HOSTS";
pub const ALLOWED_ORIGINS_ENV: &str = "MESHY_DCC_GATEWAY_ALLOWED_ORIGINS";

/// `host[:port]` → lowercase host without brackets or port.
fn host_name(authority: &str) -> String {
    let authority = authority.trim().to_ascii_lowercase();
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split(']').next().unwrap_or_default().to_string();
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && port.bytes().all(|b| b.is_ascii_digit()) => {
            host.to_string()
        }
        _ => authority,
    }
}

fn is_loopback_name(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn listed(env: &str, value: &str) -> bool {
    std::env::var(env).is_ok_and(|list| {
        list.split(',')
            .map(str::trim)
            .any(|item| item == "*" || item.eq_ignore_ascii_case(value))
    })
}

fn host_allowed(authority: &str) -> bool {
    let host = host_name(authority);
    host.parse::<IpAddr>().is_ok() || is_loopback_name(&host) || listed(ALLOWED_HOSTS_ENV, &host)
}

/// Whether a browser `Origin` may talk to a gateway reached as `host`.
pub fn origin_allowed(origin: &str, host: Option<&str>) -> bool {
    if listed(ALLOWED_ORIGINS_ENV, origin.trim_end_matches('/')) {
        return true;
    }
    let Some(authority) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false; // `null`, `file://`, extension schemes
    };
    let authority = authority.trim_end_matches('/');
    let Some(host) = host else {
        return false;
    };
    authority.eq_ignore_ascii_case(host.trim())
        || (is_loopback_name(&host_name(authority)) && is_loopback_name(&host_name(host)))
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// CORS that answers only the origins [`origin_guard`] accepts.
pub fn cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowOrigin, Any, CorsLayer};
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(cors_origin_allowed))
        .allow_methods(Any)
        .allow_headers(Any)
}

/// The CORS predicate: reflect only origins the guard would let through.
pub fn cors_origin_allowed(origin: &HeaderValue, parts: &axum::http::request::Parts) -> bool {
    origin
        .to_str()
        .is_ok_and(|origin| origin_allowed(origin, header_str(&parts.headers, header::HOST)))
}

pub async fn origin_guard(req: Request<Body>, next: Next) -> Response {
    let headers = req.headers();
    let origin = header_str(headers, header::ORIGIN);
    let from_browser = origin.is_some() || headers.contains_key("sec-fetch-site");
    if from_browser {
        let host = header_str(headers, header::HOST);
        if !host.is_some_and(host_allowed) {
            return blocked(format!(
                "this address is not allowed from a browser; open the gateway by IP address or localhost, or add the host name to {ALLOWED_HOSTS_ENV}"
            ));
        }
        if let Some(origin) = origin
            && !origin_allowed(origin, host)
        {
            return blocked(format!(
                "requests from {origin} are not allowed; add it to {ALLOWED_ORIGINS_ENV} on the gateway to allow it"
            ));
        }
    }
    next.run(req).await
}

fn blocked(message: String) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(json!({"ok": false, "error": "origin_not_allowed", "message": message})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_origin_and_loopback_pages_are_allowed() {
        assert!(origin_allowed(
            "http://127.0.0.1:19765",
            Some("127.0.0.1:19765")
        ));
        assert!(origin_allowed(
            "http://localhost:19765",
            Some("127.0.0.1:19765")
        ));
        assert!(origin_allowed(
            "http://127.0.0.1:5173",
            Some("localhost:19765")
        ));
        assert!(origin_allowed(
            "http://192.168.1.20:19765",
            Some("192.168.1.20:19765")
        ));
        assert!(origin_allowed("http://[::1]:19765", Some("[::1]:19765")));
    }

    #[test]
    fn other_sites_are_refused() {
        assert!(!origin_allowed(
            "https://evil.example",
            Some("127.0.0.1:19765")
        ));
        assert!(!origin_allowed("null", Some("127.0.0.1:19765")));
        assert!(!origin_allowed(
            "http://192.168.1.9:19765",
            Some("192.168.1.20:19765")
        ));
        assert!(!origin_allowed("http://127.0.0.1:19765", None));
    }

    #[test]
    fn rebindable_host_names_are_refused() {
        assert!(host_allowed("127.0.0.1:19765"));
        assert!(host_allowed("localhost:19765"));
        assert!(host_allowed("10.0.0.5:19765"));
        assert!(host_allowed("[::1]:19765"));
        assert!(!host_allowed("evil.example:19765"));
        assert!(!host_allowed("studio-pc.local:19765"));
    }
}
