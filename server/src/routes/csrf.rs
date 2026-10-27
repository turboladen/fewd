use axum::extract::Request;
use axum::http::{header, HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;

/// Rejects with 403 any POST a browser could send cross-site without a CORS
/// preflight. A POST passes only when its Content-Type is `application/json`
/// (parameters and case ignored) or it carries an `X-Requested-With` header.
/// Every other method passes untouched.
//
// An HTML form, `fetch` and `sendBeacon` can all POST
// cross-site with no preflight, but only with a CORS-safelisted content type
// (urlencoded, multipart, text/plain) or none, and never with a custom header.
// A request that passes here is one the browser had to preflight, and the
// server grants no CORS (see main.rs), so that preflight fails from any other
// origin. The test is an allowlist that fails closed: a missing or empty
// Content-Type, or any essence but `application/json`, is rejected, so a value
// the browser parses differently from us is refused.
pub async fn reject_cors_simple_posts(request: Request, next: Next) -> Response {
    if request.method() != Method::POST || is_json_or_marked(request.headers()) {
        return next.run(request).await;
    }
    let header_text = |name: &str| {
        request
            .headers()
            .get(name)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
    };
    tracing::warn!(
        method = %request.method(),
        path = request.uri().path(),
        origin = ?header_text("origin"),
        sec_fetch_site = ?header_text("sec-fetch-site"),
        "rejected a POST that is neither JSON nor marked with X-Requested-With"
    );
    AppError::Forbidden(
        "POST rejected as a possible cross-site request: send Content-Type: \
         application/json or an X-Requested-With header"
            .into(),
    )
    .into_response()
}

// Reports whether the request declares a JSON body or carries
// X-Requested-With. Only these two shapes pass. Some other shapes would also
// force a preflight, but refusing them costs nothing, because no client sends
// them.
fn is_json_or_marked(headers: &HeaderMap) -> bool {
    if headers.contains_key("x-requested-with") {
        return true;
    }
    let Some(value) = headers.get(header::CONTENT_TYPE) else {
        return false;
    };
    let bytes = value.as_bytes();
    let essence = match bytes.iter().position(|&b| b == b';') {
        Some(end) => &bytes[..end],
        None => bytes,
    };
    essence
        .trim_ascii()
        .eq_ignore_ascii_case(b"application/json")
}
